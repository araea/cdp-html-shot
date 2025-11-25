
# cdp-html-shot

[<img alt="github" src="https://img.shields.io/badge/github-araea/cdp_html_shot-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/araea/cdp-html-shot)
[<img alt="crates.io" src="https://img.shields.io/crates/v/cdp-html-shot.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/cdp-html-shot)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-cdp_html_shot-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/cdp-html-shot)

A high-performance Rust library for capturing HTML screenshots using the Chrome DevTools Protocol (CDP).

- **Robust**: Automatic cleanup of browser processes and temporary files (RAII).
- **Fast**: Asynchronous API built on `tokio` and WebSockets.
- **Precise**: Capture screenshots of specific DOM elements via CSS selectors.
- **HiDPI Support**: Control `deviceScaleFactor` for crystal-clear, high-resolution images.
- **Flexible**: Full control over viewport, image format, quality, and more.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
cdp-html-shot = "0.2"
```

## Examples

### Quick Capture

Render HTML strings and capture specific elements instantly.

```rust
use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;

#[tokio::main]
async fn main() -> Result<()> {
    let html = r#"
        <html>
            <body>
                <h1 id="title">Hello, CDP!</h1>
            </body>
        </html>
    "#;

    // Launch headless browser
    let browser = Browser::new().await?;

    // Render and capture the <h1> element
    let base64_image = browser.capture_html(html, "#title").await?;

    // Decode and save
    let img_data = base64::prelude::BASE64_STANDARD.decode(base64_image)?;
    std::fs::write("screenshot.jpeg", img_data)?;

    Ok(())
}
```

### HiDPI Screenshots

Capture high-resolution images using `deviceScaleFactor` (similar to Puppeteer's `page.setViewport()`).

```rust
use anyhow::Result;
use base64::Engine;
use cdp_html_shot::{Browser, CaptureOptions, ImageFormat, Viewport};

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;
    let html = "<h1 style='font-size:48px'>Crystal Clear!</h1>";

    // Method 1: Quick HiDPI capture (2x resolution)
    let base64_image = browser.capture_html_hidpi(html, "h1", 2.0).await?;

    // Method 2: Full control with CaptureOptions
    let options = CaptureOptions::new()
        .with_format(ImageFormat::Png)
        .with_viewport(
            Viewport::new(1920, 1080)
                .with_device_scale_factor(3.0) // 3x for ultra-sharp images
        )
        .with_omit_background(true); // Transparent background

    let base64_image = browser
        .capture_html_with_options(html, "h1", options)
        .await?;

    // Decode and save
    let img_data = base64::prelude::BASE64_STANDARD.decode(base64_image)?;
    std::fs::write("hidpi_screenshot.png", img_data)?;

    Ok(())
}
```

### Advanced Tab Control

Manually manage tabs, viewport, navigation, and element selection for complex scenarios.

```rust
use anyhow::Result;
use base64::Engine;
use cdp_html_shot::{Browser, CaptureOptions, Viewport};

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;
    let tab = browser.new_tab().await?;

    // Set viewport with HiDPI scaling
    tab.set_viewport(
        &Viewport::new(1280, 720)
            .with_device_scale_factor(2.0)
            .with_mobile(false),
    )
    .await?;

    // Inject content
    tab.set_content("<h1>Complex Report</h1><div class='chart'>...</div>")
        .await?;

    // Wait for dynamic element and capture
    let element = tab.wait_for_selector(".chart", 5000).await?;
    let base64_image = element
        .screenshot_with_options(CaptureOptions::raw_png())
        .await?;

    // Execute JavaScript
    let title = tab.evaluate_as_string("document.title").await?;
    println!("Page title: {}", title);

    // Take full page screenshot
    let page_screenshot = tab
        .screenshot(CaptureOptions::high_quality_jpeg())
        .await?;

    // Cleanup
    tab.close().await?;
    browser.close_async().await?;

    Ok(())
}
```

### Viewport Configuration

The `Viewport` struct provides full control over page dimensions and device emulation:

```rust
use cdp_html_shot::Viewport;

// Simple viewport
let viewport = Viewport::new(1920, 1080);

// HiDPI viewport (2x sharper images)
let viewport = Viewport::new(1920, 1080)
    .with_device_scale_factor(2.0);

// Mobile emulation
let viewport = Viewport::new(375, 812)
    .with_device_scale_factor(3.0)
    .with_mobile(true)
    .with_touch(true);

// Using the builder pattern
let viewport = Viewport::builder()
    .width(1440)
    .height(900)
    .device_scale_factor(2.0)
    .is_mobile(false)
    .build();
```

### Capture Options

Fine-tune screenshot output with `CaptureOptions`:

```rust
use cdp_html_shot::{CaptureOptions, ImageFormat, Viewport};

// PNG with transparency
let opts = CaptureOptions::new()
    .with_format(ImageFormat::Png)
    .with_omit_background(true);

// High-quality JPEG
let opts = CaptureOptions::new()
    .with_format(ImageFormat::Jpeg)
    .with_quality(95);

// Convenience presets
let opts = CaptureOptions::raw_png();
let opts = CaptureOptions::high_quality_jpeg();
let opts = CaptureOptions::hidpi();       // 2x scale
let opts = CaptureOptions::ultra_hidpi(); // 3x scale
```

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
