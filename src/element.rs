use crate::tab::Tab;
use crate::transport::next_id;
use crate::types::{CaptureOptions, ImageFormat};
use crate::utils::{self, send_and_get_msg};
use anyhow::{Context, Result};
use serde_json::json;

/// Represents a DOM element controlled via CDP.
pub struct Element<'a> {
    parent: &'a Tab,
    backend_node_id: u64,
}

impl<'a> Element<'a> {
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

    pub async fn screenshot(&self) -> Result<String> {
        self.screenshot_with_options(CaptureOptions::new().with_quality(90))
            .await
    }

    pub async fn raw_screenshot(&self) -> Result<String> {
        self.screenshot_with_options(CaptureOptions::raw_png())
            .await
    }

    pub async fn screenshot_with_options(&self, opts: CaptureOptions) -> Result<String> {
        if let Some(ref viewport) = opts.viewport {
            self.parent.set_viewport(viewport).await?;
        }

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
            "format": opts.format.as_str(),
            "clip": { "x": x, "y": y, "width": w, "height": h, "scale": 1.0 },
            "fromSurface": true,
            "captureBeyondViewport": opts.full_page,
        });

        if matches!(opts.format, ImageFormat::Jpeg | ImageFormat::WebP) {
            params["quality"] = json!(opts.quality.unwrap_or(90));
        }

        if opts.omit_background && matches!(opts.format, ImageFormat::Png) {
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

    pub fn backend_node_id(&self) -> u64 {
        self.backend_node_id
    }
}
