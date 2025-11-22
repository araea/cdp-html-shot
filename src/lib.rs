/*!
[![GitHub]](https://github.com/araea/cdp-html-shot)&ensp;[![crates-io]](https://crates.io/crates/cdp-html-shot)&ensp;[![docs-rs]](crate)

[GitHub]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
[crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
[docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs

<br>

A Rust library for capturing HTML screenshots using the Chrome DevTools Protocol (CDP).
*/

pub use browser::Browser;
pub use element::Element;
#[cfg(feature = "atexit")]
pub use exit_hook::ExitHook;
pub use tab::Tab;

/// Configuration options for HTML screenshot capture.
#[derive(Debug, Clone, Default)]
pub struct CaptureOptions {
    pub(crate) raw_png: bool,
}

impl CaptureOptions {
    /// Creates a new `CaptureOptions` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Specifies whether to capture screenshots as raw PNG (`true`) or JPEG (`false`).
    pub fn with_raw_png(mut self, raw: bool) -> Self {
        self.raw_png = raw;
        self
    }
}

// ==========================================
// Module: Transport
// ==========================================
mod transport {
    use anyhow::{Result, anyhow};
    use futures_util::stream::{SplitSink, SplitStream};
    use futures_util::{SinkExt, StreamExt};
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::net::TcpStream;
    use tokio::sync::{mpsc, oneshot};
    use tokio::time;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

    /// Global atomic ID counter for generating unique message IDs.
    pub(crate) static GLOBAL_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Returns a unique incremental ID for request messages.
    pub(crate) fn next_id() -> usize {
        GLOBAL_ID_COUNTER.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Messages sent to the transport actor.
    #[derive(Debug)]
    pub(crate) enum TransportMessage {
        /// A request command with a response sender.
        Request(Value, oneshot::Sender<Result<TransportResponse>>),
        /// Listener for target messages with given ID.
        ListenTargetMessage(u64, oneshot::Sender<Result<TransportResponse>>),
        /// Command to shut down the transport.
        Shutdown,
    }

    /// Responses produced by the transport actor.
    #[derive(Debug)]
    pub(crate) enum TransportResponse {
        Response(Response),
        Target(TargetMessage),
    }

    /// Represents a generic CDP response.
    #[derive(Debug, Serialize, Deserialize)]
    pub(crate) struct Response {
        pub(crate) id: u64,
        pub(crate) result: Value,
    }

    /// Represents messages sent from targets (such as received target messages).
    #[derive(Debug, Serialize, Deserialize)]
    pub(crate) struct TargetMessage {
        pub(crate) params: Value,
    }

    /// Internal transport actor managing WebSocket communication and request-response handling.
    struct TransportActor {
        pending_requests: HashMap<u64, oneshot::Sender<Result<TransportResponse>>>,
        ws_sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        command_rx: mpsc::Receiver<TransportMessage>,
    }

    impl TransportActor {
        /// Event loop handling incoming/outgoing WebSocket messages and commands.
        async fn run(
            mut self,
            mut ws_stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        ) {
            loop {
                tokio::select! {
                    Some(msg) = ws_stream.next() => {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(response) = serde_json::from_str::<Response>(&text) {
                                    if let Some(sender) = self.pending_requests.remove(&response.id) {
                                        let _ = sender.send(Ok(TransportResponse::Response(response)));
                                    }
                                } else if let Ok(target_msg) = serde_json::from_str::<TargetMessage>(&text) {
                                    // Handle "Target.receivedMessageFromTarget" notifications.
                                    if let Some(inner_str) = target_msg.params.get("message").and_then(|v| v.as_str())
                                        && let Ok(inner_json) = serde_json::from_str::<Value>(inner_str)
                                            && let Some(id) = inner_json.get("id").and_then(|i| i.as_u64())
                                                && let Some(sender) = self.pending_requests.remove(&id) {
                                                    let _ = sender.send(Ok(TransportResponse::Target(target_msg)));
                                                }
                                }
                            }
                            Err(_) => break,
                            _ => {}
                        }
                    }
                    Some(msg) = self.command_rx.recv() => {
                        match msg {
                            TransportMessage::Request(cmd, tx) => {
                                if let Some(id) = cmd["id"].as_u64()
                                    && let Ok(text) = serde_json::to_string(&cmd) {
                                        if self.ws_sink.send(Message::Text(text)).await.is_ok() {
                                            self.pending_requests.insert(id, tx);
                                        } else {
                                            let _ = tx.send(Err(anyhow!("WebSocket send failed")));
                                        }
                                    }
                            },
                            TransportMessage::ListenTargetMessage(id, tx) => {
                                self.pending_requests.insert(id, tx);
                            },
                            TransportMessage::Shutdown => {
                                let _ = self.ws_sink.send(Message::Text(json!({
                                    "id": next_id(),
                                    "method": "Browser.close",
                                    "params": {}
                                }).to_string())).await;
                                let _ = self.ws_sink.close().await;
                                break;
                            }
                        }
                    }
                    else => break,
                }
            }
        }
    }

    /// Asynchronous transport interface to the Chrome DevTools Protocol over WebSocket.
    #[derive(Debug)]
    pub(crate) struct Transport {
        tx: mpsc::Sender<TransportMessage>,
    }

    impl Transport {
        /// Creates a new transport connected to the specified WebSocket URL.
        pub(crate) async fn new(ws_url: &str) -> Result<Self> {
            let (ws_stream, _) = connect_async(ws_url).await?;
            let (ws_sink, ws_stream) = ws_stream.split();
            let (tx, rx) = mpsc::channel(100);

            tokio::spawn(async move {
                let actor = TransportActor {
                    pending_requests: HashMap::new(),
                    ws_sink,
                    command_rx: rx,
                };
                actor.run(ws_stream).await;
            });

            Ok(Self { tx })
        }

        /// Sends a command and awaits its response.
        pub(crate) async fn send(&self, command: Value) -> Result<TransportResponse> {
            let (tx, rx) = oneshot::channel();
            self.tx
                .send(TransportMessage::Request(command, tx))
                .await
                .map_err(|_| anyhow!("Transport actor dropped"))?;
            time::timeout(Duration::from_secs(10), rx)
                .await
                .map_err(|_| anyhow!("Timeout waiting for response"))?
                .map_err(|_| anyhow!("Response channel closed"))?
        }

        /// Waits for a specific target message by ID.
        pub(crate) async fn get_target_msg(&self, msg_id: usize) -> Result<TransportResponse> {
            let (tx, rx) = oneshot::channel();
            self.tx
                .send(TransportMessage::ListenTargetMessage(msg_id as u64, tx))
                .await
                .map_err(|_| anyhow!("Transport actor dropped"))?;
            time::timeout(Duration::from_secs(10), rx)
                .await
                .map_err(|_| anyhow!("Timeout waiting for target message"))?
                .map_err(|_| anyhow!("Response channel closed"))?
        }

        /// Initiates a graceful shutdown of the transport.
        pub(crate) async fn shutdown(&self) {
            let _ = self.tx.send(TransportMessage::Shutdown).await;
        }
    }
}

// ==========================================
// Module: Utilities
// ==========================================
mod utils {
    use crate::transport::{TargetMessage, Transport, TransportResponse, next_id};
    use anyhow::{Result, anyhow};
    use serde_json::{Value, json};
    use std::sync::Arc;

    /// Parses the contained JSON message string from a `TargetMessage`.
    pub(crate) fn serde_msg(msg: &TargetMessage) -> Result<Value> {
        let str_msg = msg.params["message"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid message format"))?;
        Ok(serde_json::from_str(str_msg)?)
    }

    /// Sends a message to a target and waits for the corresponding response.
    pub(crate) async fn send_and_get_msg(
        transport: Arc<Transport>,
        msg_id: usize,
        session_id: &str,
        msg: String,
    ) -> Result<TargetMessage> {
        let send_fut = transport.send(json!({
            "id": next_id(),
            "method": "Target.sendMessageToTarget",
            "params": { "sessionId": session_id, "message": msg }
        }));
        let recv_fut = transport.get_target_msg(msg_id);

        let (_, target_msg) = futures_util::try_join!(send_fut, recv_fut)?;

        match target_msg {
            TransportResponse::Target(res) => Ok(res),
            other => Err(anyhow!("Unexpected response: {:?}", other)),
        }
    }
}

// ==========================================
// Module: Element
// ==========================================
mod element {
    use crate::tab::Tab;
    use crate::transport::next_id;
    use crate::utils::{self, send_and_get_msg};
    use anyhow::{Context, Result};
    use serde_json::json;

    /// Represents a DOM element controlled via CDP.
    pub struct Element<'a> {
        parent: &'a Tab,
        backend_node_id: u64,
    }

    impl<'a> Element<'a> {
        /// Constructs a new element from a node ID, fetching necessary info.
        pub(crate) async fn new(parent: &'a Tab, node_id: u64) -> Result<Self> {
            let msg_id = next_id();
            let msg = json!({
                "id": msg_id,
                "method": "DOM.describeNode",
                "params": { "nodeId": node_id, "depth": 100 }
            })
            .to_string();

            let res =
                send_and_get_msg(parent.transport.clone(), msg_id, &parent.session_id, msg).await?;
            let data = utils::serde_msg(&res)?;
            let backend_node_id = data["result"]["node"]["backendNodeId"]
                .as_u64()
                .context("Missing backendNodeId")?;

            Ok(Self {
                parent,
                backend_node_id,
            })
        }

        /// Captures a JPEG screenshot of the element.
        pub async fn screenshot(&self) -> Result<String> {
            self.take_screenshot("jpeg", Some(90)).await
        }

        /// Captures a raw PNG screenshot of the element.
        pub async fn raw_screenshot(&self) -> Result<String> {
            self.take_screenshot("png", None).await
        }

        /// Internal function to capture a screenshot with specified format and quality.
        async fn take_screenshot(&self, format: &str, quality: Option<u8>) -> Result<String> {
            let msg_id = next_id();
            let msg_box = json!({
                "id": msg_id,
                "method": "DOM.getBoxModel",
                "params": { "backendNodeId": self.backend_node_id }
            })
            .to_string();

            let res_box = send_and_get_msg(
                self.parent.transport.clone(),
                msg_id,
                &self.parent.session_id,
                msg_box,
            )
            .await?;
            let data_box = utils::serde_msg(&res_box)?;
            let border = &data_box["result"]["model"]["border"];

            let (x, y, w, h) = (
                border[0].as_f64().unwrap_or(0.0),
                border[1].as_f64().unwrap_or(0.0),
                (border[2].as_f64().unwrap_or(0.0) - border[0].as_f64().unwrap_or(0.0)),
                (border[5].as_f64().unwrap_or(0.0) - border[1].as_f64().unwrap_or(0.0)),
            );

            let mut params = json!({
                "format": format,
                "clip": { "x": x, "y": y, "width": w, "height": h, "scale": 1.0 },
                "fromSurface": true,
                "captureBeyondViewport": true,
            });
            if let Some(q) = quality {
                params["quality"] = json!(q);
            }

            let msg_id = next_id();
            let msg_cap = json!({
                "id": msg_id,
                "method": "Page.captureScreenshot",
                "params": params
            })
            .to_string();

            self.parent.activate().await?;
            let res_cap = send_and_get_msg(
                self.parent.transport.clone(),
                msg_id,
                &self.parent.session_id,
                msg_cap,
            )
            .await?;
            let data_cap = utils::serde_msg(&res_cap)?;

            data_cap["result"]["data"]
                .as_str()
                .map(|s| s.to_string())
                .context("No image data received")
        }
    }
}

// ==========================================
// Module: Tab
// ==========================================
mod tab {
    use crate::element::Element;
    use crate::transport::{Transport, TransportResponse, next_id};
    use crate::utils::{self, send_and_get_msg};
    use anyhow::{Context, Result, anyhow};
    use serde_json::json;
    use std::sync::Arc;

    /// Represents a CDP browser tab (target) session.
    pub struct Tab {
        pub(crate) transport: Arc<Transport>,
        pub(crate) session_id: String,
        pub(crate) target_id: String,
    }

    impl Tab {
        /// Creates a new blank tab and attaches to it.
        pub(crate) async fn new(transport: Arc<Transport>) -> Result<Self> {
            let TransportResponse::Response(res_create) = transport
                .send(json!({ "id": next_id(), "method": "Target.createTarget", "params": { "url": "about:blank" } }))
                .await? else { return Err(anyhow!("Invalid response type")); };

            let target_id = res_create.result["targetId"]
                .as_str()
                .context("No targetId")?
                .to_string();

            let TransportResponse::Response(res_attach) = transport
                .send(json!({ "id": next_id(), "method": "Target.attachToTarget", "params": { "targetId": target_id } }))
                .await? else { return Err(anyhow!("Invalid response type")); };

            let session_id = res_attach.result["sessionId"]
                .as_str()
                .context("No sessionId")?
                .to_string();

            Ok(Self {
                transport,
                session_id,
                target_id,
            })
        }

        /// Sets the tab's HTML content and waits until page stability.
        pub async fn set_content(&self, content: &str) -> Result<&Self> {
            let js_check = r#"
            (async()=>{try{const b=30000,c=200,d=500,e=Date.now();await new Promise((f,g)=>{let h=null,i=null,j=Date.now();const k=()=>{i&&i.disconnect(),h&&clearTimeout(h),f(!0)},l=async()=>{if(Date.now()-e>b){return i&&i.disconnect(),void g(new Error("Timeout"))}if("complete"!==document.readyState)return setTimeout(l,100);await document.fonts.ready;const m=[...Array.from(document.querySelectorAll('link[rel="stylesheet"]')),...Array.from(document.images)].filter(n=>"LINK"===n.tagName?!n.sheet:"IMG"===n.tagName&&!n.complete);return m.length>0?setTimeout(l,100):void o()},o=()=>{if(!i){j=Date.now(),i=new MutationObserver(p=>{j=Date.now()}),i.observe(document.documentElement,{childList:!0,subtree:!0,attributes:!0,characterData:!0});const p=()=>{const q=Date.now(),r=q-e,s=q-j;return r>b?k():s>=c&&r>=d?void requestAnimationFrame(()=>{requestAnimationFrame(()=>{k()})}):void setTimeout(p,100)};p()}};document.open(),document.write(CONTENT_PLACEHOLDER),document.close(),l()});return"Page stable"}catch(t){throw new Error(t.message)}})();
            "#.replace("CONTENT_PLACEHOLDER", &serde_json::to_string(content)?);

            let msg_id = next_id();
            let msg = json!({
                "id": msg_id,
                "method": "Runtime.evaluate",
                "params": { "expression": js_check, "awaitPromise": true, "returnByValue": true }
            })
            .to_string();

            send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
            Ok(self)
        }

        /// Finds the first element matching the given CSS selector.
        pub async fn find_element(&self, selector: &str) -> Result<Element<'_>> {
            let msg_id = next_id();
            let msg_doc =
                json!({ "id": msg_id, "method": "DOM.getDocument", "params": {} }).to_string();
            let res_doc =
                send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg_doc).await?;
            let data_doc = utils::serde_msg(&res_doc)?;
            let root_node_id = data_doc["result"]["root"]["nodeId"]
                .as_u64()
                .context("No root node")?;

            let msg_id = next_id();
            let msg_sel = json!({
                "id": msg_id,
                "method": "DOM.querySelector",
                "params": { "nodeId": root_node_id, "selector": selector }
            })
            .to_string();
            let res_sel =
                send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg_sel).await?;
            let data_sel = utils::serde_msg(&res_sel)?;
            let node_id = data_sel["result"]["nodeId"]
                .as_u64()
                .context("Element not found")?;

            Element::new(self, node_id).await
        }

        /// Activates the target tab to bring it to the foreground.
        pub async fn activate(&self) -> Result<&Self> {
            let msg_id = next_id();
            let msg = json!({ "id": msg_id, "method": "Target.activateTarget", "params": { "targetId": self.target_id } }).to_string();
            send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
            Ok(self)
        }

        /// Navigates the tab to the specified URL.
        pub async fn goto(&self, url: &str) -> Result<&Self> {
            let msg_id = next_id();
            let msg = json!({ "id": msg_id, "method": "Page.navigate", "params": { "url": url } })
                .to_string();
            send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
            Ok(self)
        }

        /// Closes the target tab.
        pub async fn close(&self) -> Result<()> {
            let msg_id = next_id();
            let msg = json!({ "id": msg_id, "method": "Target.closeTarget", "params": { "targetId": self.target_id } }).to_string();
            send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
            Ok(())
        }
    }
}

// ==========================================
// Module: Browser (FIXED & OPTIMIZED)
// ==========================================
mod browser {
    use crate::transport::{Transport, TransportResponse, next_id};
    use crate::{CaptureOptions, Tab};
    use anyhow::{Context, Result, anyhow};
    use rand::{Rng, thread_rng};
    use regex::Regex;
    use serde_json::json;
    use std::io::{BufRead, BufReader};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use which::which;

    /// Temporary directory for browser user data, deleted on drop.
    struct CustomTempDir {
        path: PathBuf,
    }

    impl CustomTempDir {
        /// Creates a new temporary directory with timestamp and random suffix.
        fn new(base: PathBuf, prefix: &str) -> Result<Self> {
            std::fs::create_dir_all(&base)?;
            let name = format!(
                "{}_{}_{}",
                prefix,
                chrono::Local::now().format("%Y%m%d_%H%M%S"),
                thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(6)
                    .map(char::from)
                    .collect::<String>()
            );
            let path = base.join(name);
            std::fs::create_dir(&path)?;
            Ok(Self { path })
        }
    }

    impl Drop for CustomTempDir {
        /// Attempts to delete the temporary directory, retrying on failure.
        fn drop(&mut self) {
            for _ in 0..3 {
                if std::fs::remove_dir_all(&self.path).is_ok() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Holds the browser process and associated temporary directory.
    /// Responsible for killing process and cleaning up on drop.
    struct BrowserProcess {
        child: Child,
        _temp: CustomTempDir,
    }

    impl Drop for BrowserProcess {
        /// Ensures the child process is killed and waited upon before dropping temp.
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            // `_temp` will be dropped automatically thereafter, deleting the temp dir.
        }
    }

    #[derive(Clone)]
    pub struct Browser {
        transport: Arc<Transport>,
        /// Manages the browser process, wrapped in a mutex for async access and optional ownership.
        process: Arc<Mutex<Option<BrowserProcess>>>,
    }

    static GLOBAL_BROWSER: Mutex<Option<Browser>> = Mutex::const_new(None);

    impl Browser {
        /// Launches a new headless browser instance.
        pub async fn new() -> Result<Self> {
            Self::launch(true).await
        }

        /// Launches a new browser instance with head visible.
        pub async fn new_with_head() -> Result<Self> {
            Self::launch(false).await
        }

        /// Internal function to start the browser with given headless flag.
        async fn launch(headless: bool) -> Result<Self> {
            let temp = CustomTempDir::new(std::env::current_dir()?.join("temp"), "cdp-shot")?;
            let exe = Self::find_chrome()?;
            let port = (8000..9000)
                .find(|&p| std::net::TcpListener::bind(("127.0.0.1", p)).is_ok())
                .ok_or(anyhow!("No available port"))?;

            let mut args = vec![
                format!("--remote-debugging-port={}", port),
                format!("--user-data-dir={}", temp.path.display()),
                "--no-sandbox".into(), "--no-zygote".into(), "--in-process-gpu".into(),
                "--disable-dev-shm-usage".into(), "--disable-background-networking".into(),
                "--disable-default-apps".into(), "--disable-extensions".into(),
                "--disable-sync".into(), "--disable-translate".into(),
                "--metrics-recording-only".into(), "--safebrowsing-disable-auto-update".into(),
                "--mute-audio".into(), "--no-first-run".into(), "--hide-scrollbars".into(),
                "--window-size=1200,1600".into(),
                "--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".into()
            ];
            if headless {
                args.push("--headless=new".into());
            }

            #[cfg(windows)]
            let mut cmd = {
                use std::os::windows::process::CommandExt;
                let mut c = Command::new(&exe);
                c.creation_flags(0x08000000); // CREATE_NO_WINDOW
                c
            };
            #[cfg(not(windows))]
            let mut cmd = Command::new(&exe);

            let mut child = cmd.args(args).stderr(Stdio::piped()).spawn()?;
            let stderr = child.stderr.take().context("No stderr")?;
            let ws_url = Self::wait_for_ws(stderr).await?;

            Ok(Self {
                transport: Arc::new(Transport::new(&ws_url).await?),
                process: Arc::new(Mutex::new(Some(BrowserProcess { child, _temp: temp }))),
            })
        }

        /// Attempts to locate a Chrome or Edge executable in the system.
        fn find_chrome() -> Result<PathBuf> {
            if let Ok(p) = std::env::var("CHROME") {
                return Ok(p.into());
            }
            let apps = [
                "google-chrome-stable",
                "chromium",
                "chrome",
                "msedge",
                "microsoft-edge",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            ];
            for app in apps {
                if let Ok(p) = which(app) {
                    return Ok(p);
                }
                if Path::new(app).exists() {
                    return Ok(app.into());
                }
            }

            #[cfg(windows)]
            {
                use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};
                let keys = [
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
                ];
                for k in keys {
                    if let Ok(rk) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(k)
                        && let Ok(v) = rk.get_value::<String, _>("")
                    {
                        return Ok(v.into());
                    }
                }
                let paths = [
                    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                ];
                for p in paths {
                    if Path::new(p).exists() {
                        return Ok(p.into());
                    }
                }
            }
            Err(anyhow!("Chrome/Edge not found. Set CHROME env var."))
        }

        /// Reads browser stderr lines to extract the WebSocket debugging URL.
        async fn wait_for_ws(stderr: std::process::ChildStderr) -> Result<String> {
            let reader = BufReader::new(stderr);
            let re = Regex::new(r"listening on (.*/devtools/browser/.*)$")?;
            tokio::task::spawn_blocking(move || {
                for line in reader.lines() {
                    let l = line?;
                    if let Some(cap) = re.captures(&l) {
                        return Ok(cap[1].to_string());
                    }
                }
                Err(anyhow!("WS URL not found in stderr"))
            })
            .await?
        }

        /// Opens a new blank tab.
        pub async fn new_tab(&self) -> Result<Tab> {
            Tab::new(self.transport.clone()).await
        }

        /// Captures a screenshot of HTML content clipped to the given selector using default options.
        pub async fn capture_html(&self, html: &str, selector: &str) -> Result<String> {
            self.capture_html_with_options(html, selector, CaptureOptions::default())
                .await
        }

        /// Captures a screenshot with options such as raw PNG or JPEG.
        pub async fn capture_html_with_options(
            &self,
            html: &str,
            selector: &str,
            opts: CaptureOptions,
        ) -> Result<String> {
            let tab = self.new_tab().await?;
            tab.set_content(html).await?;
            let el = tab.find_element(selector).await?;
            let shot = if opts.raw_png {
                el.raw_screenshot().await?
            } else {
                el.screenshot().await?
            };
            let _ = tab.close().await;
            Ok(shot)
        }

        /// Closes the browser process and cleans up resources asynchronously.
        pub async fn close_async(&self) -> Result<()> {
            self.transport.shutdown().await;
            let mut lock = self.process.lock().await;
            if let Some(_proc) = lock.take() {
                // Drop triggers process kill, wait and temp dir removal.
            }
            Ok(())
        }

        /// Returns a shared singleton browser instance, launching if necessary.
        pub async fn instance() -> Self {
            let mut lock = GLOBAL_BROWSER.lock().await;
            if let Some(b) = &*lock {
                return b.clone();
            }
            let b = Self::new().await.expect("Init global browser failed");
            if let Ok(TransportResponse::Response(res)) = b
                .transport
                .send(json!({"id": next_id(), "method":"Target.getTargets", "params":{}}))
                .await
                && let Some(list) = res.result["targetInfos"].as_array()
                && let Some(id) = list
                    .iter()
                    .find(|t| t["type"] == "page")
                    .and_then(|t| t["targetId"].as_str())
            {
                let _ = b.transport.send(json!({"id":next_id(), "method":"Target.closeTarget", "params":{"targetId":id}})).await;
            }
            *lock = Some(b.clone());
            b
        }
    }
}

// ==========================================
// Module: Exit Hook
// ==========================================
#[cfg(feature = "atexit")]
mod exit_hook {
    use std::sync::{Arc, Once};

    /// Registers a function to be called on program exit or Ctrl+C signal.
    pub struct ExitHook {
        func: Arc<dyn Fn() + Send + Sync>,
    }

    impl ExitHook {
        /// Creates a new exit hook with the specified closure.
        pub fn new<F: Fn() + Send + Sync + 'static>(f: F) -> Self {
            Self { func: Arc::new(f) }
        }

        /// Registers the hook to run on Ctrl+C, guaranteeing single registration.
        pub fn register(&self) -> Result<(), Box<dyn std::error::Error>> {
            static ONCE: Once = Once::new();
            let f = self.func.clone();
            let mut res = Ok(());
            ONCE.call_once(|| {
                if let Err(e) = ctrlc::set_handler(move || {
                    f();
                    std::process::exit(0);
                }) {
                    eprintln!("Ctrl+C handler error: {}", e);
                }
            });
            res
        }
    }

    impl Drop for ExitHook {
        /// Calls the registered exit function on drop.
        fn drop(&mut self) {
            (self.func)();
        }
    }
}
