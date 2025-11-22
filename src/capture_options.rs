/// Configuration options for HTML capture.
#[derive(Debug, Clone, Default)]
pub struct CaptureOptions {
    pub(crate) raw_png: bool,
}

impl CaptureOptions {
    /// Create new capture options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to use a raw PNG format (true) or JPEG (false).
    pub fn with_raw_png(mut self, raw: bool) -> Self {
        self.raw_png = raw;
        self
    }
}
