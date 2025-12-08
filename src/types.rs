/// Viewport configuration for controlling page dimensions and device emulation.
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

    pub fn with_device_scale_factor(mut self, factor: f64) -> Self {
        self.device_scale_factor = factor;
        self
    }

    pub fn with_mobile(mut self, is_mobile: bool) -> Self {
        self.is_mobile = is_mobile;
        self
    }

    pub fn with_touch(mut self, has_touch: bool) -> Self {
        self.has_touch = has_touch;
        self
    }

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
    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn device_scale_factor(mut self, factor: f64) -> Self {
        self.device_scale_factor = Some(factor);
        self
    }

    pub fn is_mobile(mut self, mobile: bool) -> Self {
        self.is_mobile = Some(mobile);
        self
    }

    pub fn has_touch(mut self, touch: bool) -> Self {
        self.has_touch = Some(touch);
        self
    }

    pub fn is_landscape(mut self, landscape: bool) -> Self {
        self.is_landscape = Some(landscape);
        self
    }

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
    #[default]
    Jpeg,
    Png,
    WebP,
}

impl ImageFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Png => "png",
            ImageFormat::WebP => "webp",
        }
    }
}

/// Defines a rectangular region for clipping screenshots.
#[derive(Debug, Clone, Copy)]
pub struct ClipRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

impl ClipRegion {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            scale: 1.0,
        }
    }

    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
}

/// Configuration options for HTML screenshot capture.
#[derive(Debug, Clone, Default)]
pub struct CaptureOptions {
    pub(crate) format: ImageFormat,
    pub(crate) quality: Option<u8>,
    pub(crate) viewport: Option<Viewport>,
    pub(crate) full_page: bool,
    pub(crate) omit_background: bool,
    pub(crate) clip: Option<ClipRegion>,
}

impl CaptureOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_format(mut self, format: ImageFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = Some(quality.min(100));
        self
    }

    pub fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = Some(viewport);
        self
    }

    pub fn with_full_page(mut self, full_page: bool) -> Self {
        self.full_page = full_page;
        self
    }

    pub fn with_omit_background(mut self, omit: bool) -> Self {
        self.omit_background = omit;
        self
    }

    pub fn with_clip(mut self, clip: ClipRegion) -> Self {
        self.clip = Some(clip);
        self
    }

    pub fn raw_png() -> Self {
        Self::new().with_format(ImageFormat::Png)
    }

    pub fn high_quality_jpeg() -> Self {
        Self::new().with_format(ImageFormat::Jpeg).with_quality(95)
    }

    pub fn hidpi() -> Self {
        Self::new().with_viewport(Viewport::default().with_device_scale_factor(2.0))
    }

    pub fn ultra_hidpi() -> Self {
        Self::new().with_viewport(Viewport::default().with_device_scale_factor(3.0))
    }

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
