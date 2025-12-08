use crate::element::Element;
use crate::transport::{Transport, TransportResponse, next_id};
use crate::types::{CaptureOptions, ImageFormat, Viewport};
use crate::utils::{self, send_and_get_msg};
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

/// Represents a CDP browser tab (target) session.
pub struct Tab {
    pub(crate) transport: Arc<Transport>,
    pub(crate) session_id: String,
    pub(crate) target_id: String,
}

impl Tab {
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

    pub(crate) async fn send_cmd(&self, method: &str, params: serde_json::Value) -> Result<Value> {
        let msg_id = next_id();
        let msg = json!({
            "id": msg_id,
            "method": method,
            "params": params
        })
        .to_string();
        let res = send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
        utils::serde_msg(&res)
    }

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

    pub async fn clear_viewport(&self) -> Result<&Self> {
        self.send_cmd("Emulation.clearDeviceMetricsOverride", json!({}))
            .await?;
        Ok(self)
    }

    pub async fn set_content(&self, content: &str) -> Result<&Self> {
        self.send_cmd("Page.enable", json!({})).await?;

        // Register listener BEFORE triggering the event to avoid race conditions
        let event_rx = self
            .transport
            .listen_for_event(&self.session_id, "Page.loadEventFired")
            .await?;

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

        time::timeout(Duration::from_secs(30), event_rx)
            .await
            .map_err(|_| anyhow!("Timeout waiting for event Page.loadEventFired"))?
            .map_err(|_| anyhow!("Event channel closed"))?;

        Ok(self)
    }

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

    pub async fn evaluate_as_string(&self, expression: &str) -> Result<String> {
        let value = self.evaluate(expression).await?;
        value
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| Some(value.to_string()))
            .context("Failed to convert result to string")
    }

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

    pub async fn wait_for_selector(&self, selector: &str, timeout_ms: u64) -> Result<Element<'_>> {
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

    pub async fn screenshot(&self, opts: CaptureOptions) -> Result<String> {
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

    pub async fn activate(&self) -> Result<&Self> {
        let msg_id = next_id();
        let msg = json!({ "id": msg_id, "method": "Target.activateTarget", "params": { "targetId": self.target_id } }).to_string();
        send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
        Ok(self)
    }

    pub async fn goto(&self, url: &str) -> Result<&Self> {
        self.send_cmd("Page.enable", json!({})).await?;

        let event_rx = self
            .transport
            .listen_for_event(&self.session_id, "Page.loadEventFired")
            .await?;

        let msg_id = next_id();
        let msg = json!({ "id": msg_id, "method": "Page.navigate", "params": { "url": url } })
            .to_string();
        send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;

        time::timeout(Duration::from_secs(30), event_rx)
            .await
            .map_err(|_| anyhow!("Timeout waiting for event Page.loadEventFired"))?
            .map_err(|_| anyhow!("Event channel closed"))?;

        Ok(self)
    }

    pub async fn goto_no_wait(&self, url: &str) -> Result<&Self> {
        let msg_id = next_id();
        let msg = json!({ "id": msg_id, "method": "Page.navigate", "params": { "url": url } })
            .to_string();
        send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
        Ok(self)
    }

    pub async fn reload(&self) -> Result<&Self> {
        self.send_cmd("Page.enable", json!({})).await?;

        let event_rx = self
            .transport
            .listen_for_event(&self.session_id, "Page.loadEventFired")
            .await?;

        self.send_cmd("Page.reload", json!({})).await?;

        time::timeout(Duration::from_secs(30), event_rx)
            .await
            .map_err(|_| anyhow!("Timeout waiting for event Page.loadEventFired"))?
            .map_err(|_| anyhow!("Event channel closed"))?;

        Ok(self)
    }

    pub async fn url(&self) -> Result<String> {
        self.evaluate_as_string("window.location.href").await
    }

    pub async fn title(&self) -> Result<String> {
        self.evaluate_as_string("document.title").await
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    pub async fn close(&self) -> Result<()> {
        let msg_id = next_id();
        let msg = json!({ "id": msg_id, "method": "Target.closeTarget", "params": { "targetId": self.target_id } }).to_string();
        send_and_get_msg(self.transport.clone(), msg_id, &self.session_id, msg).await?;
        Ok(())
    }
}
