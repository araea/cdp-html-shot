# cdp-html-shot

[<img alt="github" src="https://img.shields.io/badge/github-araea/cdp_html_shot-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/araea/cdp-html-shot)
[<img alt="crates.io" src="https://img.shields.io/crates/v/cdp-html-shot.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/cdp-html-shot)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-cdp_html_shot-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/cdp-html-shot)

A high-performance Rust library for capturing HTML screenshots using the Chrome DevTools Protocol (CDP).

- **Robust**: Automatic cleanup of browser processes and temporary files (RAII).
- **Fast**: Asynchronous API built on `tokio` and WebSockets.
- **Precise**: Capture screenshots of specific DOM elements via CSS selectors.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
cdp-html-shot = "0.1"
```

## Examples

### Quick Capture
Render HTML strings and capture specific elements instantly.

```rust
use base64::Engine;
use anyhow::Result;
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
    let png_base64 = browser.capture_html(html, "#title").await?;

    // Decode and save
    let img_data = base64::prelude::BASE64_STANDARD.decode(png_base64)?;
    std::fs::write("screenshot.jpeg", img_data)?;

    Ok(())
}
```

### Advanced Control
Manually manage tabs, navigation, and element selection for complex scenarios.

```rust
use base64::Engine;
use anyhow::Result;
use cdp_html_shot::Browser;

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;
    let tab = browser.new_tab().await?;

    // Inject content and wait for stabilization
    tab.set_content("<h1>Complex Report</h1><div class='chart'>...</div>").await?;

    // Locate element and capture
    let element = tab.find_element(".chart").await?;
    let base64 = element.screenshot().await?;
    
    // Cleanup tab
    tab.close().await?;

    Ok(())
}
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
