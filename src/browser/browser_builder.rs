use anyhow::Result;

use crate::Browser;
use crate::browser::browser_config::BrowserConfig;

/// Builder for configuring and creating Browser instances.
pub struct BrowserBuilder {
    config: BrowserConfig,
}

impl BrowserBuilder {
    /// Create a new BrowserBuilder with default configuration.
    pub fn new() -> Self {
        Self {
            config: BrowserConfig::new().expect("Failed to create default browser config"),
        }
    }

    /// Set whether the browser should run in headless mode.
    pub fn headless(mut self, headless: bool) -> Self {
        self.config.headless = headless;
        self
    }

    // Configure additional options here as needed.
    // pub fn with_option(mut self, option: Option) -> Self { ... }

    /// Build and launch the browser with the configured options.
    pub async fn build(self) -> Result<Browser> {
        Browser::create_browser(self.config).await
    }
}

impl Default for BrowserBuilder {
    fn default() -> Self {
        Self::new()
    }
}
