mod browser_builder;
mod browser_config;
mod browser_utils;
mod temp_dir;

use anyhow::{Context, Result, anyhow};
use browser_config::BrowserConfig;
use log::warn;
use serde_json::json;
use std::process::Child;
use std::sync::{Arc, Mutex};
use temp_dir::CustomTempDir;
use tokio::sync::Mutex as AsyncMutex;

use crate::CaptureOptions;
use crate::browser::browser_builder::BrowserBuilder;
use crate::general_utils::next_id;
use crate::tab::Tab;
use crate::transport::Transport;
use crate::transport_actor::TransportResponse;

static GLOBAL_BROWSER: AsyncMutex<Option<Arc<Browser>>> = AsyncMutex::const_new(None);

#[derive(Debug)]
struct Process {
    child: Child,
    _temp_dir: CustomTempDir,
}

/// A browser instance.
#[derive(Debug)]
pub struct Browser {
    transport: Arc<Transport>,
    process: Mutex<Option<Process>>,
}

impl Browser {
    /// Create a new browser instance with default configuration (headless).
    pub async fn new() -> Result<Self> {
        BrowserBuilder::new().build().await
    }

    /// Create a new browser instance with a visible window.
    pub async fn new_with_head() -> Result<Self> {
        BrowserBuilder::new().headless(false).build().await
    }

    /// Create browser instance with custom configuration.
    async fn create_browser(config: BrowserConfig) -> Result<Self> {
        let mut child = browser_utils::spawn_chrome_process(&config)?;
        let stderr = child
            .stderr
            .take()
            .context("Failed to get stderr from Chrome process")?;

        let ws_url = browser_utils::get_websocket_url(stderr).await?;

        Ok(Self {
            transport: Arc::new(Transport::new(&ws_url).await?),
            process: Mutex::new(Some(Process {
                child,
                _temp_dir: config.temp_dir,
            })),
        })
    }

    pub async fn new_tab(&self) -> Result<Tab> {
        Tab::new(self.transport.clone()).await
    }

    /// Close the initial tab created when the browser starts.
    pub async fn close_init_tab(&self) -> Result<()> {
        let response = self
            .transport
            .send(json!({
                "id": next_id(),
                "method": "Target.getTargets",
                "params": {}
            }))
            .await?;

        let TransportResponse::Response(res) = response else {
            return Err(anyhow!("Unexpected response type when getting targets"));
        };

        let target_infos = res
            .result
            .get("targetInfos")
            .and_then(|t| t.as_array())
            .context("Invalid targetInfos format")?;

        let target_id = target_infos
            .iter()
            .find(|info| info.get("type").and_then(|t| t.as_str()) == Some("page"))
            .and_then(|info| info.get("targetId"))
            .and_then(|id| id.as_str())
            .context("Could not find initial page target")?;

        self.transport
            .send(json!({
                "id": next_id(),
                "method": "Target.closeTarget",
                "params": {
                    "targetId": target_id
                }
            }))
            .await?;

        Ok(())
    }

    pub async fn capture_html(&self, html: &str, selector: &str) -> Result<String> {
        self.capture_html_with_options(html, selector, CaptureOptions::default())
            .await
    }

    pub async fn capture_html_with_options(
        &self,
        html: &str,
        selector: &str,
        options: CaptureOptions,
    ) -> Result<String> {
        let tab = self.new_tab().await?;

        let result = async {
            tab.set_content(html).await?;
            let element = tab.find_element(selector).await?;
            if options.raw_png {
                element.raw_screenshot().await
            } else {
                element.screenshot().await
            }
        }
        .await;

        if let Err(e) = tab.close().await {
            warn!("Failed to close tab after capture: {:?}", e);
        }

        result
    }

    /**
    Close the browser.
    */
    pub fn close(&self) -> Result<()> {
        // 1. Shutdown Transport
        self.transport.shutdown();

        // 2. Kill Process
        let mut process_guard = self
            .process
            .lock()
            .map_err(|_| anyhow!("Failed to lock browser process"))?;

        if let Some(mut process) = process_guard.take() {
            process
                .child
                .kill()
                .context("Failed to kill browser process")?;
            process
                .child
                .wait()
                .context("Failed to wait for browser process exit")?;
        }

        Ok(())
    }
}

// ==========================================
// 单例模式实现
// ==========================================
impl Browser {
    /// Get the global Browser instance.
    pub async fn instance() -> Arc<Browser> {
        let mut lock = GLOBAL_BROWSER.lock().await;

        if let Some(browser) = &*lock {
            return browser.clone();
        }

        let browser = Browser::new()
            .await
            .expect("Failed to initialize global browser");

        if let Err(e) = browser.close_init_tab().await {
            warn!("Failed to close initial tab: {}", e);
        }

        let browser_arc = Arc::new(browser);
        *lock = Some(browser_arc.clone());

        browser_arc
    }

    /// Close the global Browser instance.
    pub async fn close_instance() -> Result<()> {
        let mut lock = GLOBAL_BROWSER.lock().await;

        if let Some(browser) = lock.take() {
            browser.close()?;
        }
        Ok(())
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        if let Err(e) = self.close()
            && !e.to_string().contains("Failed to lock")
        {
            warn!("Error closing browser in Drop: {:?}", e);
        }
    }
}
