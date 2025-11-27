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

/// Viewport configuration for controlling page dimensions and device emulation.
///
/// Similar to Puppeteer's `page.setViewport()`, this allows you to control
/// the page dimensions and device scale factor for higher quality screenshots.
///
/// # Example
/// ```rust,ignore
/// use cdp_html_shot::Viewport;
///
/// // Create a high-DPI viewport for sharper images
/// let viewport = Viewport::new(1920, 1080)
///     .with_device_scale_factor(2.0);
///
/// // Or use builder pattern for full control
/// let viewport = Viewport::builder()
///     .width(1280)
///     .height(720)
///     .device_scale_factor(3.0)
///     .is_mobile(false)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct Viewport {
    /// Viewport width in pixels.
    pub width: u32,
    /// Viewport height in pixels.
    pub height: u32,
    /// Device scale factor (DPR). Higher values (e.g., 2.0, 3.0) produce sharper images.
    /// Default is 1.0.
    pub device_scale_factor: f64,
    /// Whether to emulate a mobile device. Default is false.
    pub is_mobile: bool,
    /// Whether touch events are supported. Default is false.
    pub has_touch: bool,
    /// Whether viewport is in landscape mode. Default is false.
    pub is_landscape: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            device_scale_factor: 1.0,
            is_mobile: false,
            has_touch: false,
            is_landscape: false,
        }
    }
}

impl Viewport {
    /// Creates a new viewport with specified dimensions and default settings.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Creates a new viewport builder for fluent configuration.
    pub fn builder() -> ViewportBuilder {
        ViewportBuilder::default()
    }

    /// Sets the device scale factor (DPR) for higher quality images.
    ///
    /// Common values:
    /// - 1.0: Standard resolution
    /// - 2.0: Retina/HiDPI (2x sharper)
    /// - 3.0: Ultra-high DPI (3x sharper)
    pub fn with_device_scale_factor(mut self, factor: f64) -> Self {
        self.device_scale_factor = factor;
        self
    }

    /// Sets whether to emulate a mobile device.
    pub fn with_mobile(mut self, is_mobile: bool) -> Self {
        self.is_mobile = is_mobile;
        self
    }

    /// Sets whether touch events are supported.
    pub fn with_touch(mut self, has_touch: bool) -> Self {
        self.has_touch = has_touch;
        self
    }

    /// Sets whether the viewport is in landscape mode.
    pub fn with_landscape(mut self, is_landscape: bool) -> Self {
        self.is_landscape = is_landscape;
        self
    }
}

/// Builder for creating Viewport configurations with a fluent API.
#[derive(Debug, Clone, Default)]
pub struct ViewportBuilder {
    width: Option<u32>,
    height: Option<u32>,
    device_scale_factor: Option<f64>,
    is_mobile: Option<bool>,
    has_touch: Option<bool>,
    is_landscape: Option<bool>,
}

impl ViewportBuilder {
    /// Sets the viewport width in pixels.
    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets the viewport height in pixels.
    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    /// Sets the device scale factor (DPR).
    pub fn device_scale_factor(mut self, factor: f64) -> Self {
        self.device_scale_factor = Some(factor);
        self
    }

    /// Sets whether to emulate a mobile device.
    pub fn is_mobile(mut self, mobile: bool) -> Self {
        self.is_mobile = Some(mobile);
        self
    }

    /// Sets whether touch events are supported.
    pub fn has_touch(mut self, touch: bool) -> Self {
        self.has_touch = Some(touch);
        self
    }

    /// Sets whether viewport is in landscape mode.
    pub fn is_landscape(mut self, landscape: bool) -> Self {
        self.is_landscape = Some(landscape);
        self
    }

    /// Builds the Viewport with configured or default values.
    pub fn build(self) -> Viewport {
        let default = Viewport::default();
        Viewport {
            width: self.width.unwrap_or(default.width),
            height: self.height.unwrap_or(default.height),
            device_scale_factor: self
                .device_scale_factor
                .unwrap_or(default.device_scale_factor),
            is_mobile: self.is_mobile.unwrap_or(default.is_mobile),
            has_touch: self.has_touch.unwrap_or(default.has_touch),
            is_landscape: self.is_landscape.unwrap_or(default.is_landscape),
        }
    }
}

/// Screenshot format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFormat {
    /// JPEG format (smaller file size, lossy compression).
    #[default]
    Jpeg,
    /// PNG format (lossless, supports transparency).
    Png,
    /// WebP format (modern format with good compression).
    WebP,
}

impl ImageFormat {
    /// Returns the format string used by CDP.
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Png => "png",
            ImageFormat::WebP => "webp",
        }
    }
}

/// Configuration options for HTML screenshot capture.
///
/// Provides fine-grained control over the screenshot capture process,
/// including image format, quality, viewport settings, and more.
///
/// # Example
/// ```rust,ignore
/// use cdp_html_shot::{CaptureOptions, Viewport, ImageFormat};
///
/// let options = CaptureOptions::new()
///     .with_format(ImageFormat::Png)
///     .with_viewport(Viewport::new(1920, 1080).with_device_scale_factor(2.0))
///     .with_full_page(true);
/// ```
#[derive(Debug, Clone, Default)]
pub struct CaptureOptions {
    /// Image format for the screenshot.
    pub(crate) format: ImageFormat,
    /// Quality for JPEG/WebP (0-100). Ignored for PNG.
    pub(crate) quality: Option<u8>,
    /// Viewport settings to apply before capture.
    pub(crate) viewport: Option<Viewport>,
    /// Whether to capture the full scrollable page.
    pub(crate) full_page: bool,
    /// Whether to omit the background (transparent for PNG).
    pub(crate) omit_background: bool,
    /// Optional clip region for the screenshot.
    pub(crate) clip: Option<ClipRegion>,
}

/// Defines a rectangular region for clipping screenshots.
#[derive(Debug, Clone, Copy)]
pub struct ClipRegion {
    /// X coordinate of the clip region.
    pub x: f64,
    /// Y coordinate of the clip region.
    pub y: f64,
    /// Width of the clip region.
    pub width: f64,
    /// Height of the clip region.
    pub height: f64,
    /// Scale factor for the clip region.
    pub scale: f64,
}

impl ClipRegion {
    /// Creates a new clip region with the specified dimensions.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            scale: 1.0,
        }
    }

    /// Sets the scale factor for the clip region.
    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
}

impl CaptureOptions {
    /// Creates a new `CaptureOptions` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the image format for the screenshot.
    pub fn with_format(mut self, format: ImageFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets the quality for JPEG/WebP (0-100). Ignored for PNG.
    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = Some(quality.min(100));
        self
    }

    /// Sets the viewport configuration for the capture.
    ///
    /// This is particularly useful for setting `deviceScaleFactor` to get
    /// higher resolution screenshots.
    pub fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = Some(viewport);
        self
    }

    /// Sets whether to capture the full scrollable page.
    pub fn with_full_page(mut self, full_page: bool) -> Self {
        self.full_page = full_page;
        self
    }

    /// Sets whether to omit the background (transparent for PNG).
    pub fn with_omit_background(mut self, omit: bool) -> Self {
        self.omit_background = omit;
        self
    }

    /// Sets a clip region for the screenshot.
    pub fn with_clip(mut self, clip: ClipRegion) -> Self {
        self.clip = Some(clip);
        self
    }

    /// Convenience method: creates options for raw PNG output.
    pub fn raw_png() -> Self {
        Self::new().with_format(ImageFormat::Png)
    }

    /// Convenience method: creates options for high-quality JPEG.
    pub fn high_quality_jpeg() -> Self {
        Self::new().with_format(ImageFormat::Jpeg).with_quality(95)
    }

    /// Convenience method: creates options for HiDPI (2x) screenshots.
    pub fn hidpi() -> Self {
        Self::new().with_viewport(Viewport::default().with_device_scale_factor(2.0))
    }

    /// Convenience method: creates options for ultra HiDPI (3x) screenshots.
    pub fn ultra_hidpi() -> Self {
        Self::new().with_viewport(Viewport::default().with_device_scale_factor(3.0))
    }

    // Legacy compatibility method
    /// Specifies whether to capture screenshots as raw PNG (`true`) or JPEG (`false`).
    #[deprecated(since = "0.2.0", note = "Use `with_format()` instead")]
    pub fn with_raw_png(mut self, raw: bool) -> Self {
        self.format = if raw {
            ImageFormat::Png
        } else {
            ImageFormat::Jpeg
        };
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

    /// Global counter for generating unique message IDs
    pub(crate) static GLOBAL_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Generates the next unique message ID
    pub(crate) fn next_id() -> usize {
        GLOBAL_ID_COUNTER.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Messages that can be sent to the transport actor
    #[derive(Debug)]
    pub(crate) enum TransportMessage {
        /// Send a CDP command and wait for response
        Request(Value, oneshot::Sender<Result<TransportResponse>>),
        /// Listen for a specific target message by ID
        ListenTargetMessage(u64, oneshot::Sender<Result<TransportResponse>>),
        /// Wait for a specific CDP event
        WaitForEvent(String, String, oneshot::Sender<()>),
        /// Shutdown the transport and browser
        Shutdown,
    }

    /// Responses received from the transport
    #[derive(Debug)]
    pub(crate) enum TransportResponse {
        /// Direct response from CDP
        Response(Response),
        /// Message from target session
        Target(TargetMessage),
    }

    /// Standard CDP response format
    #[derive(Debug, Serialize, Deserialize)]
    pub(crate) struct Response {
        /// Message ID matching the request
        pub(crate) id: u64,
        /// Response data
        pub(crate) result: Value,
    }

    /// Message received from target session
    #[derive(Debug, Serialize, Deserialize)]
    pub(crate) struct TargetMessage {
        /// Message parameters
        pub(crate) params: Value,
    }

    /// Actor that manages WebSocket communication with Chrome DevTools Protocol
    struct TransportActor {
        /// Pending requests waiting for responses
        pending_requests: HashMap<u64, oneshot::Sender<Result<TransportResponse>>>,
        /// Event listeners waiting for specific CDP events
        event_listeners: HashMap<(String, String), Vec<oneshot::Sender<()>>>,
        /// WebSocket sink for sending messages
        ws_sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        /// Channel for receiving commands
        command_rx: mpsc::Receiver<TransportMessage>,
    }

    impl TransportActor {
        /// Main event loop for processing WebSocket messages and commands
        async fn run(
            mut self,
            mut ws_stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        ) {
            loop {
                tokio::select! {
                    Some(msg) = ws_stream.next() => {
                        match msg {
                            Ok(Message::Text(text)) => {
                                // Try to parse as regular Response
                                if let Ok(response) = serde_json::from_str::<Response>(&text) {
                                    if let Some(sender) = self.pending_requests.remove(&response.id) {
                                        let _ = sender.send(Ok(TransportResponse::Response(response)));
                                    }
                                }
                                // Try to parse as TargetMessage (from Target.receivedMessageFromTarget)
                                else if let Ok(target_msg) = serde_json::from_str::<TargetMessage>(&text)
                                    && let Some(inner_str) = target_msg.params.get("message").and_then(|v| v.as_str())
                                        && let Ok(inner_json) = serde_json::from_str::<Value>(inner_str) {

                                            // Case A: This is a response to a request (has ID)
                                            if let Some(id) = inner_json.get("id").and_then(|i| i.as_u64()) {
                                                if let Some(sender) = self.pending_requests.remove(&id) {
                                                    let _ = sender.send(Ok(TransportResponse::Target(target_msg)));
                                                }
                                            }
                                            // Case B: This is an event (no ID, has Method) -> trigger listeners
                                            else if let Some(method) = inner_json.get("method").and_then(|s| s.as_str())
                                                && let Some(session_id) = target_msg.params.get("sessionId").and_then(|s| s.as_str()) {
                                                    let key = (session_id.to_string(), method.to_string());
                                                    if let Some(senders) = self.event_listeners.remove(&key) {
                                                        for tx in senders {
                                                            let _ = tx.send(());
                                                        }
                                                    }
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
                            // Handle event listener registration
                            TransportMessage::WaitForEvent(session_id, method, tx) => {
                                self.event_listeners.entry((session_id, method)).or_default().push(tx);
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

    /// WebSocket transport for Chrome DevTools Protocol communication
    #[derive(Debug)]
    pub(crate) struct Transport {
        /// Channel for sending commands to the transport actor
        tx: mpsc::Sender<TransportMessage>,
    }

    impl Transport {
        /// Creates a new transport connection to the WebSocket URL
        pub(crate) async fn new(ws_url: &str) -> Result<Self> {
            let (ws_stream, _) = connect_async(ws_url).await?;
            let (ws_sink, ws_stream) = ws_stream.split();
            let (tx, rx) = mpsc::channel(100);

            tokio::spawn(async move {
                let actor = TransportActor {
                    pending_requests: HashMap::new(),
                    event_listeners: HashMap::new(),
                    ws_sink,
                    command_rx: rx,
                };
                actor.run(ws_stream).await;
            });

            Ok(Self { tx })
        }

        /// Sends a CDP command and waits for the response
        pub(crate) async fn send(&self, command: Value) -> Result<TransportResponse> {
            let (tx, rx) = oneshot::channel();
            self.tx
                .send(TransportMessage::Request(command, tx))
                .await
                .map_err(|_| anyhow!("Transport actor dropped"))?;
            time::timeout(Duration::from_secs(30), rx)
                .await
                .map_err(|_| anyhow!("Timeout waiting for response"))?
                .map_err(|_| anyhow!("Response channel closed"))?
        }

        /// Waits for a specific target message by ID
        pub(crate) async fn get_target_msg(&self, msg_id: usize) -> Result<TransportResponse> {
            let (tx, rx) = oneshot::channel();
            self.tx
                .send(TransportMessage::ListenTargetMessage(msg_id as u64, tx))
                .await
                .map_err(|_| anyhow!("Transport actor dropped"))?;
            time::timeout(Duration::from_secs(30), rx)
                .await
                .map_err(|_| anyhow!("Timeout waiting for target message"))?
                .map_err(|_| anyhow!("Response channel closed"))?
        }

        /// Waits for a specific CDP event from a session
        pub(crate) async fn wait_for_event(&self, session_id: &str, method: &str) -> Result<()> {
            let (tx, rx) = oneshot::channel();
            self.tx
                .send(TransportMessage::WaitForEvent(
                    session_id.to_string(),
                    method.to_string(),
                    tx,
                ))
                .await
                .map_err(|_| anyhow!("Transport actor dropped"))?;

            time::timeout(Duration::from_secs(30), rx)
                .await
                .map_err(|_| anyhow!("Timeout waiting for event {}", method))?
                .map_err(|_| anyhow!("Event channel closed"))?;
            Ok(())
        }

        /// Shuts down the transport and browser
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
    use crate::{CaptureOptions, ImageFormat};
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

        /// Captures a JPEG screenshot of the element with default quality.
        pub async fn screenshot(&self) -> Result<String> {
            self.screenshot_with_options(CaptureOptions::new().with_quality(90))
                .await
        }

        /// Captures a raw PNG screenshot of the element.
        pub async fn raw_screenshot(&self) -> Result<String> {
            self.screenshot_with_options(CaptureOptions::raw_png())
                .await
        }

        /// Captures a screenshot of the element with custom options.
        ///
        /// # Example
        /// ```rust,ignore
        /// let options = CaptureOptions::new()
        ///     .with_format(ImageFormat::Png)
        ///     .with_viewport(Viewport::new(1920, 1080).with_device_scale_factor(2.0));
        ///
        /// let base64_image = element.screenshot_with_options(options).await?;
        /// ```
        pub async fn screenshot_with_options(&self, opts: CaptureOptions) -> Result<String> {
            // Apply viewport if specified
            if let Some(ref viewport) = opts.viewport {
                self.parent.set_viewport(viewport).await?;
            }

            // Get element bounding box
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

            // Build screenshot params
            let mut params = json!({
                "format": opts.format.as_str(),
                "clip": { "x": x, "y": y, "width": w, "height": h, "scale": 1.0 },
                "fromSurface": true,
                "captureBeyondViewport": opts.full_page,
            });

            // Add quality for JPEG/WebP
            if matches!(opts.format, ImageFormat::Jpeg | ImageFormat::WebP) {
                params["quality"] = json!(opts.quality.unwrap_or(90));
            }

            // Handle transparent background for PNG
            if opts.omit_background && matches!(opts.format, ImageFormat::Png) {
                // Enable transparent background
                let msg_id = next_id();
                let msg = json!({
                    "id": msg_id,
                    "method": "Emulation.setDefaultBackgroundColorOverride",
                    "params": { "color": { "r": 0, "g": 0, "b": 0, "a": 0 } }
                })
                .to_string();
                send_and_get_msg(
                    self.parent.transport.clone(),
                    msg_id,
                    &self.parent.session_id,
                    msg,
                )
                .await?;
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

            // Reset background color override if we changed it
            if opts.omit_background && matches!(opts.format, ImageFormat::Png) {
                let msg_id = next_id();
                let msg = json!({
                    "id": msg_id,
                    "method": "Emulation.setDefaultBackgroundColorOverride",
                    "params": {}
                })
                .to_string();
                let _ = send_and_get_msg(
                    self.parent.transport.clone(),
                    msg_id,
                    &self.parent.session_id,
                    msg,
                )
                .await;
            }

            data_cap["result"]["data"]
                .as_str()
                .map(|s| s.to_string())
                .context("No image data received")
        }

        /// Returns the backend node ID of this element.
        pub fn backend_node_id(&self) -> u64 {
            self.backend_node_id
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
    use crate::{CaptureOptions, ImageFormat, Viewport};
    use anyhow::{Context, Result, anyhow};
    use serde_json::{Value, json};
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

        /// Helper function: sends a command to Target and waits for response.
        pub(crate) async fn send_cmd(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> Result<Value> {
            let msg_id = next_id();
            let msg = json!({
                "id": msg_id,
                "method": method,
                "params": params
            })
            .to_string();
            let res =
                send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
            utils::serde_msg(&res)
        }

        /// Sets the viewport size and device scale factor.
        ///
        /// This is similar to Puppeteer's `page.setViewport()` and is essential
        /// for getting higher resolution screenshots via `deviceScaleFactor`.
        ///
        /// # Example
        /// ```rust,ignore
        /// let viewport = Viewport::new(1920, 1080)
        ///     .with_device_scale_factor(2.0);  // 2x resolution for sharper images
        ///
        /// tab.set_viewport(&viewport).await?;
        /// ```
        pub async fn set_viewport(&self, viewport: &Viewport) -> Result<&Self> {
            let screen_orientation = if viewport.is_landscape {
                json!({"type": "landscapePrimary", "angle": 90})
            } else {
                json!({"type": "portraitPrimary", "angle": 0})
            };

            self.send_cmd(
                "Emulation.setDeviceMetricsOverride",
                json!({
                    "width": viewport.width,
                    "height": viewport.height,
                    "deviceScaleFactor": viewport.device_scale_factor,
                    "mobile": viewport.is_mobile,
                    "screenOrientation": screen_orientation
                }),
            )
            .await?;

            // Set touch emulation if needed
            if viewport.has_touch {
                self.send_cmd(
                    "Emulation.setTouchEmulationEnabled",
                    json!({
                        "enabled": true,
                        "maxTouchPoints": 5
                    }),
                )
                .await?;
            }

            Ok(self)
        }

        /// Clears the viewport override, returning to default browser behavior.
        pub async fn clear_viewport(&self) -> Result<&Self> {
            self.send_cmd("Emulation.clearDeviceMetricsOverride", json!({}))
                .await?;
            Ok(self)
        }

        /// Sets HTML content and waits for the "load" event.
        pub async fn set_content(&self, content: &str) -> Result<&Self> {
            // 1. Enable Page domain to ensure lifecycle events are sent
            self.send_cmd("Page.enable", json!({})).await?;

            // 2. Prepare to wait for `load` event
            let load_event_future = self
                .transport
                .wait_for_event(&self.session_id, "Page.loadEventFired");

            // 3. Execute document.write
            let js_write = format!(
                r#"document.open(); document.write({}); document.close();"#,
                serde_json::to_string(content)?
            );

            self.send_cmd(
                "Runtime.evaluate",
                json!({
                    "expression": js_write,
                    "awaitPromise": true
                }),
            )
            .await?;

            // 4. Wait for the event to trigger
            load_event_future.await?;

            Ok(self)
        }

        /// Evaluates JavaScript in the page context and returns the result.
        ///
        /// # Example
        /// ```rust,ignore
        /// let result = tab.evaluate("document.title").await?;
        /// println!("Page title: {}", result);
        /// ```
        pub async fn evaluate(&self, expression: &str) -> Result<Value> {
            let result = self
                .send_cmd(
                    "Runtime.evaluate",
                    json!({
                        "expression": expression,
                        "returnByValue": true,
                        "awaitPromise": true
                    }),
                )
                .await?;
            Ok(result["result"]["result"]["value"].clone())
        }

        /// Evaluates JavaScript and returns the result as a string.
        pub async fn evaluate_as_string(&self, expression: &str) -> Result<String> {
            let value = self.evaluate(expression).await?;
            value
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| Some(value.to_string()))
                .context("Failed to convert result to string")
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

        /// Waits for an element matching the selector to appear in the DOM.
        ///
        /// # Arguments
        /// * `selector` - CSS selector to wait for
        /// * `timeout_ms` - Maximum time to wait in milliseconds
        pub async fn wait_for_selector(
            &self,
            selector: &str,
            timeout_ms: u64,
        ) -> Result<Element<'_>> {
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_millis(timeout_ms);

            loop {
                match self.find_element(selector).await {
                    Ok(element) => return Ok(element),
                    Err(_) if start.elapsed() < timeout => {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        /// Captures a screenshot of the entire page.
        ///
        /// # Example
        /// ```rust,ignore
        /// let options = CaptureOptions::new()
        ///     .with_format(ImageFormat::Png)
        ///     .with_viewport(Viewport::new(1920, 1080).with_device_scale_factor(2.0));
        ///
        /// let base64_image = tab.screenshot(options).await?;
        /// ```
        pub async fn screenshot(&self, opts: CaptureOptions) -> Result<String> {
            // Apply viewport if specified
            if let Some(ref viewport) = opts.viewport {
                self.set_viewport(viewport).await?;
            }

            let mut params = json!({
                "format": opts.format.as_str(),
                "fromSurface": true,
                "captureBeyondViewport": opts.full_page,
            });

            if matches!(opts.format, ImageFormat::Jpeg | ImageFormat::WebP) {
                params["quality"] = json!(opts.quality.unwrap_or(90));
            }

            if let Some(ref clip) = opts.clip {
                params["clip"] = json!({
                    "x": clip.x,
                    "y": clip.y,
                    "width": clip.width,
                    "height": clip.height,
                    "scale": clip.scale
                });
            }

            if opts.omit_background && matches!(opts.format, ImageFormat::Png) {
                self.send_cmd(
                    "Emulation.setDefaultBackgroundColorOverride",
                    json!({ "color": { "r": 0, "g": 0, "b": 0, "a": 0 } }),
                )
                .await?;
            }

            self.activate().await?;

            let result = self.send_cmd("Page.captureScreenshot", params).await?;

            if opts.omit_background && matches!(opts.format, ImageFormat::Png) {
                let _ = self
                    .send_cmd("Emulation.setDefaultBackgroundColorOverride", json!({}))
                    .await;
            }

            result["result"]["data"]
                .as_str()
                .map(|s| s.to_string())
                .context("No image data received")
        }

        /// Activates the target tab to bring it to the foreground.
        pub async fn activate(&self) -> Result<&Self> {
            let msg_id = next_id();
            let msg = json!({ "id": msg_id, "method": "Target.activateTarget", "params": { "targetId": self.target_id } }).to_string();
            send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
            Ok(self)
        }

        /// Navigates the tab to the specified URL and waits for load.
        pub async fn goto(&self, url: &str) -> Result<&Self> {
            self.send_cmd("Page.enable", json!({})).await?;

            let load_event_future = self
                .transport
                .wait_for_event(&self.session_id, "Page.loadEventFired");

            let msg_id = next_id();
            let msg = json!({ "id": msg_id, "method": "Page.navigate", "params": { "url": url } })
                .to_string();
            send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;

            load_event_future.await?;
            Ok(self)
        }

        /// Navigates to URL without waiting for load event.
        pub async fn goto_no_wait(&self, url: &str) -> Result<&Self> {
            let msg_id = next_id();
            let msg = json!({ "id": msg_id, "method": "Page.navigate", "params": { "url": url } })
                .to_string();
            send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
            Ok(self)
        }

        /// Reloads the current page.
        pub async fn reload(&self) -> Result<&Self> {
            self.send_cmd("Page.enable", json!({})).await?;

            let load_event_future = self
                .transport
                .wait_for_event(&self.session_id, "Page.loadEventFired");

            self.send_cmd("Page.reload", json!({})).await?;

            load_event_future.await?;
            Ok(self)
        }

        /// Gets the current URL of the page.
        pub async fn url(&self) -> Result<String> {
            self.evaluate_as_string("window.location.href").await
        }

        /// Gets the page title.
        pub async fn title(&self) -> Result<String> {
            self.evaluate_as_string("document.title").await
        }

        /// Returns the session ID for this tab.
        pub fn session_id(&self) -> &str {
            &self.session_id
        }

        /// Returns the target ID for this tab.
        pub fn target_id(&self) -> &str {
            &self.target_id
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
// Module: Browser
// ==========================================
mod browser {
    use crate::transport::{Transport, TransportResponse, next_id};
    use crate::{CaptureOptions, Tab, Viewport};
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
        fn drop(&mut self) {
            // More aggressive cleanup with longer delays for Windows
            // Total max wait: ~2.4 seconds (100+200+300*8 ms)
            for i in 0..10 {
                if std::fs::remove_dir_all(&self.path).is_ok() {
                    return;
                }
                // Increasing delay: 100ms, 200ms, 300ms, 300ms, ...
                std::thread::sleep(Duration::from_millis(100 * (i as u64 + 1).min(3)));
            }
            // Final attempt - ignore error as we've done our best
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Holds the browser process and associated temporary directory.
    struct BrowserProcess {
        child: Child,
        _temp: CustomTempDir,
    }

    impl Drop for BrowserProcess {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            // Give Chrome time to release file handles before temp dir cleanup
            // This is especially important on Windows where file locks persist briefly
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
                c.creation_flags(0x08000000);
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

        /// Captures a screenshot with options such as format, quality, and viewport.
        ///
        /// # Example
        /// ```rust,ignore
        /// let options = CaptureOptions::new()
        ///     .with_format(ImageFormat::Png)
        ///     .with_viewport(Viewport::new(1920, 1080).with_device_scale_factor(2.0));
        ///
        /// let base64 = browser.capture_html_with_options(html, "body", options).await?;
        /// ```
        pub async fn capture_html_with_options(
            &self,
            html: &str,
            selector: &str,
            opts: CaptureOptions,
        ) -> Result<String> {
            let tab = self.new_tab().await?;

            // Apply viewport if specified in options
            if let Some(ref viewport) = opts.viewport {
                tab.set_viewport(viewport).await?;
            }

            tab.set_content(html).await?;
            let el = tab.find_element(selector).await?;
            let shot = el.screenshot_with_options(opts).await?;
            let _ = tab.close().await;
            Ok(shot)
        }

        /// Captures a high-DPI screenshot with the specified scale factor.
        ///
        /// Convenience method for getting sharper images without manually
        /// configuring viewport and options.
        ///
        /// # Arguments
        /// * `html` - HTML content to render
        /// * `selector` - CSS selector for the element to capture
        /// * `scale` - Device scale factor (2.0 for retina, 3.0 for ultra-high DPI)
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

        /// Gracefully shuts down the global browser instance if it exists.
        /// Does nothing if no instance is present.
        ///
        /// Intended primarily for cleanup during application shutdown.
        pub async fn shutdown_global() {
            let mut lock = GLOBAL_BROWSER.lock().await;
            // Take ownership and replace with None to drop the instance.
            if let Some(browser) = lock.take() {
                // Perform shutdown only if an instance exists.
                let _ = browser.close_async().await;
            }
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

        /// Checks if the browser connection is still alive.
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
        /// Automatically recreates the instance if it becomes invalid.
        pub async fn instance() -> Self {
            let mut lock = GLOBAL_BROWSER.lock().await;

            // Check if existing instance is still valid
            if let Some(b) = &*lock {
                if b.is_alive().await {
                    return b.clone();
                }
                // Instance is dead, clean up old process
                log::warn!("[cdp-html-shot] Browser instance died, recreating...");
                let _ = b.close_async().await;
            }

            // Recreate instance
            let b = Self::new().await.expect("Init global browser failed");

            // Close default blank page
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
            let res = Ok(());
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
        fn drop(&mut self) {
            (self.func)();
        }
    }
}
