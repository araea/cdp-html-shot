use crate::tab::Tab;
use crate::transport::{Transport, TransportResponse, next_id};
use crate::types::{CaptureOptions, Viewport};
use anyhow::{Context, Result, anyhow};
use rand::{Rng, rng};
use regex::Regex;
use serde_json::json;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use which::which;

/// Temporary directory for browser user data, deleted on drop.
struct CustomTempDir {
    path: PathBuf,
}

impl CustomTempDir {
    fn new(base: PathBuf, prefix: &str) -> Result<Self> {
        std::fs::create_dir_all(&base)?;
        let name = format!(
            "{}_{}_{}",
            prefix,
            chrono::Local::now().format("%Y%m%d_%H%M%S"),
            rng()
                .sample_iter(&rand::distr::Alphanumeric)
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
    fn drop(&mut self) {
        for i in 0..10 {
            if std::fs::remove_dir_all(&self.path).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100 * (i as u64 + 1).min(3)));
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct BrowserProcess {
    child: Child,
    _temp: CustomTempDir,
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[derive(Clone)]
pub struct Browser {
    transport: Arc<Transport>,
    process: Arc<Mutex<Option<BrowserProcess>>>,
}

static GLOBAL_BROWSER: Mutex<Option<Browser>> = Mutex::const_new(None);

impl Browser {
    /// Launches a new headless browser instance using the default browser path.
    pub async fn new() -> Result<Self> {
        Self::launch(true, None).await
    }

    /// Launches a new headless browser instance using a custom executable path.
    pub async fn new_with_path(path: impl AsRef<Path>) -> Result<Self> {
        Self::launch(true, Some(path.as_ref().to_path_buf())).await
    }

    /// Launches a new browser instance with head visible using the default browser path.
    pub async fn new_with_head() -> Result<Self> {
        Self::launch(false, None).await
    }

    /// Launches a new browser instance with head visible using a custom executable path.
    pub async fn new_with_head_and_path(path: impl AsRef<Path>) -> Result<Self> {
        Self::launch(false, Some(path.as_ref().to_path_buf())).await
    }

    /// Internal function to start the browser with given headless flag and optional path.
    async fn launch(headless: bool, custom_path: Option<PathBuf>) -> Result<Self> {
        let temp = CustomTempDir::new(std::env::current_dir()?.join("temp"), "cdp-shot")?;
        let exe = Self::find_chrome(custom_path)?;
        let port = (8000..9000)
            .find(|&p| std::net::TcpListener::bind(("127.0.0.1", p)).is_ok())
            .ok_or(anyhow!("No available port"))?;

        let mut args = vec![
            format!("--remote-debugging-port={}", port),
            format!("--user-data-dir={}", temp.path.display()),
            "--no-sandbox".into(),
            "--no-zygote".into(),
            "--in-process-gpu".into(),
            "--disable-dev-shm-usage".into(),
            "--disable-background-networking".into(),
            "--disable-default-apps".into(),
            "--disable-extensions".into(),
            "--disable-sync".into(),
            "--disable-translate".into(),
            "--metrics-recording-only".into(),
            "--safebrowsing-disable-auto-update".into(),
            "--mute-audio".into(),
            "--no-first-run".into(),
            "--hide-scrollbars".into(),
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
            c.creation_flags(0x08000000);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = Command::new(&exe);

        let mut child = cmd
            .args(args)
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn browser executable: {:?}", exe))?;

        let stderr = child.stderr.take().context("No stderr")?;
        let ws_url = Self::wait_for_ws(stderr).await?;

        Ok(Self {
            transport: Arc::new(Transport::new(&ws_url).await?),
            process: Arc::new(Mutex::new(Some(BrowserProcess { child, _temp: temp }))),
        })
    }

    /// Attempts to locate a Chrome or Edge executable in the system.
    fn find_chrome(custom_path: Option<PathBuf>) -> Result<PathBuf> {
        // 1. Try custom path if provided
        if let Some(path) = custom_path {
            if path.exists() {
                return Ok(path);
            }
            return Err(anyhow!("Custom browser path found: {:?}", path));
        }

        // 2. Try environment variable
        if let Ok(p) = std::env::var("CHROME") {
            return Ok(p.into());
        }

        // 3. Try platform specific paths
        #[cfg(target_os = "windows")]
        {
            let paths = [
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            ];
            for p in paths {
                if Path::new(p).exists() {
                    return Ok(p.into());
                }
            }

            use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};
            let keys = [
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
            ];
            for k in keys {
                if let Ok(rk) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(k)
                    && let Ok(v) = rk.get_value::<String, _>("")
                {
                    if Path::new(&v).exists() {
                        return Ok(v.into());
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let paths = [
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            ];
            for p in paths {
                if Path::new(p).exists() {
                    return Ok(p.into());
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            let paths = [
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
            ];
            for p in paths {
                if Path::new(p).exists() {
                    return Ok(p.into());
                }
            }
        }

        // 4. Try common commands using `which`
        let apps = [
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "chrome",
            "msedge",
            "microsoft-edge",
        ];
        for app in apps {
            if let Ok(p) = which(app) {
                let p_str = p.to_string_lossy();
                // Check direct path for obvious flatpak/snap markers
                if p_str.contains("/var/lib/flatpak") || p_str.contains("/snap/") {
                    continue;
                }

                // Check resolved path (in case of symlinks like /usr/bin/msedge -> /var/lib/flatpak/...)
                // Flatpak often installs a symlink in /usr/bin that points to the internal flatpak data directory.
                // We must filter these because they cannot be executed directly without `flatpak run`.
                if let Ok(resolved) = std::fs::canonicalize(&p) {
                    let r_str = resolved.to_string_lossy();
                    if r_str.contains("/var/lib/flatpak") || r_str.contains("/snap/") {
                        continue;
                    }
                }

                return Ok(p);
            }
        }

        Err(anyhow!(
            "Chrome/Edge not found. Set CHROME env var or use new_with_path."
        ))
    }

    async fn wait_for_ws(stderr: std::process::ChildStderr) -> Result<String> {
        let (tx, rx) = oneshot::channel();

        // Spawn a blocking task to read stderr.
        // Important: We loop until the stream ends (process exit) to drain stderr,
        // preventing the pipe from filling up or closing prematurely which could kill the browser.
        tokio::task::spawn_blocking(move || {
            let reader = BufReader::new(stderr);
            let re =
                Regex::new(r"listening on (.*/devtools/browser/.*)\s*$").expect("Invalid regex");
            let mut found = false;
            let mut tx = Some(tx);

            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if !found
                            && let Some(cap) = re.captures(&l) {
                                if let Some(tx) = tx.take() {
                                    let _ = tx.send(Ok(cap[1].to_string()));
                                }
                                found = true;
                            }
                    }
                    Err(_) => break,
                }
            }

            if !found
                && let Some(tx) = tx.take() {
                    let _ = tx.send(Err(anyhow!("WS URL not found in stderr")));
                }
        });

        rx.await.map_err(|_| anyhow!("Stderr reader dropped"))?
    }

    pub async fn new_tab(&self) -> Result<Tab> {
        Tab::new(self.transport.clone()).await
    }

    pub async fn capture_html(&self, html: &str, selector: &str) -> Result<String> {
        self.capture_html_with_options(html, selector, CaptureOptions::default())
            .await
    }

    pub async fn capture_html_with_options(
        &self,
        html: &str,
        selector: &str,
        opts: CaptureOptions,
    ) -> Result<String> {
        let tab = self.new_tab().await?;

        if let Some(ref viewport) = opts.viewport {
            tab.set_viewport(viewport).await?;
        }

        tab.set_content(html).await?;
        let el = tab.find_element(selector).await?;
        let shot = el.screenshot_with_options(opts).await?;
        let _ = tab.close().await;
        Ok(shot)
    }

    pub async fn capture_html_hidpi(
        &self,
        html: &str,
        selector: &str,
        scale: f64,
    ) -> Result<String> {
        let opts = CaptureOptions::new()
            .with_viewport(Viewport::default().with_device_scale_factor(scale));
        self.capture_html_with_options(html, selector, opts).await
    }

    pub async fn shutdown_global() {
        let mut lock = GLOBAL_BROWSER.lock().await;
        if let Some(browser) = lock.take() {
            let _ = browser.close_async().await;
        }
    }

    pub async fn close_async(&self) -> Result<()> {
        self.transport.shutdown().await;
        let mut lock = self.process.lock().await;
        if let Some(_proc) = lock.take() {
            // Drop triggers cleanup
        }
        Ok(())
    }

    async fn is_alive(&self) -> bool {
        self.transport
            .send(json!({
                "id": next_id(),
                "method": "Target.getTargets",
                "params": {}
            }))
            .await
            .is_ok()
    }

    /// Returns a shared singleton browser instance, launching if necessary.
    pub async fn instance() -> Self {
        Self::instance_internal(None).await
    }

    /// Returns a shared singleton browser instance, launching with the specified path if necessary.
    ///
    /// Note: If the global browser instance is already running, this path argument will be ignored
    /// and the existing instance will be returned.
    pub async fn instance_with_path(path: impl AsRef<Path>) -> Self {
        Self::instance_internal(Some(path.as_ref().to_path_buf())).await
    }

    async fn instance_internal(custom_path: Option<PathBuf>) -> Self {
        let mut lock = GLOBAL_BROWSER.lock().await;

        if let Some(b) = &*lock {
            if b.is_alive().await {
                return b.clone();
            }
            println!("[cdp-html-shot] Browser instance died, recreating...");
            let _ = b.close_async().await;
        }

        let b = Self::launch(true, custom_path)
            .await
            .expect("Init global browser failed");

        // Close default blank page to save resources
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
            let _ = b
                .transport
                .send(json!({"id":next_id(), "method":"Target.closeTarget", "params":{"targetId":id}}))
                .await;
        }

        *lock = Some(b.clone());
        b
    }
}
