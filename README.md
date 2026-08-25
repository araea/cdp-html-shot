
# cdp-html-shot

[<img alt="github" src="https://img.shields.io/badge/github-araea/cdp_html_shot-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/araea/cdp-html-shot)
[<img alt="crates.io" src="https://img.shields.io/crates/v/cdp-html-shot.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/cdp-html-shot)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-cdp_html_shot-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/cdp-html-shot)

Turn HTML into images by driving a real browser over the Chrome DevTools
Protocol. Give it a fragment of HTML and a CSS selector; get back a screenshot
of exactly that element.

- **Precise** — capture one DOM element, not a cropped page.
- **Sharp** — `deviceScaleFactor` for HiDPI output, up to any scale you like.
- **Async** — built on `tokio` and WebSockets, with no polling.
- **Tidy** — browser processes and temporary profiles are cleaned up by `Drop`.
- **Configurable** — full control over viewport, encoding, and the browser's
  own command line.
- **Portable** — Windows, macOS, Linux, and Android (Termux).

## Installation

```toml
[dependencies]
cdp-html-shot = "0.2"
```

A Chrome, Chromium, or Edge installation is required at runtime. It is located
automatically; see [Choosing a browser](#choosing-a-browser) to point the
library somewhere specific.

## Quick start

```rust
use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;

#[tokio::main]
async fn main() -> Result<()> {
    let html = "<h1 id='title' style='font: 48px sans-serif'>Hello, CDP!</h1>";

    let browser = Browser::new().await?;
    let base64 = browser.capture_html(html, "#title").await?;

    let bytes = base64::prelude::BASE64_STANDARD.decode(base64)?;
    std::fs::write("screenshot.jpeg", bytes)?;

    browser.close_async().await
}
```

Screenshots come back base64-encoded, which is what CDP itself returns — decode
it to get the image bytes.

## Capturing

### Encoding and quality

`CaptureOptions` decides how the image is encoded.

```rust
use cdp_html_shot::{CaptureOptions, ImageFormat, Viewport};

// Explicit
let options = CaptureOptions::new()
    .with_format(ImageFormat::Png)
    .with_omit_background(true)   // transparent background
    .with_full_page(true);

// Or start from a preset
let options = CaptureOptions::raw_png();
let options = CaptureOptions::high_quality_jpeg();  // JPEG, quality 95
let options = CaptureOptions::hidpi();              // 2x scale
let options = CaptureOptions::ultra_hidpi();        // 3x scale
```

`with_quality` applies to JPEG and WebP, and is clamped to the 0–100 the
protocol accepts.

### HiDPI

Raising `deviceScaleFactor` renders more pixels for the same CSS layout, the
same way `page.setViewport()` does in Puppeteer.

```rust
use cdp_html_shot::{Browser, CaptureOptions, ImageFormat, Viewport};

let browser = Browser::new().await?;
let html = "<h1 style='font-size:48px'>Crystal clear</h1>";

// Shorthand
let base64 = browser.capture_html_hidpi(html, "h1", 2.0).await?;

// Or spell it out
let options = CaptureOptions::new()
    .with_format(ImageFormat::Png)
    .with_viewport(Viewport::new(1920, 1080).with_device_scale_factor(3.0));

let base64 = browser.capture_html_with_options(html, "h1", options).await?;
```

### Viewport

```rust
use cdp_html_shot::Viewport;

let desktop = Viewport::new(1920, 1080);

let retina = Viewport::new(1920, 1080).with_device_scale_factor(2.0);

let phone = Viewport::new(375, 812)
    .with_device_scale_factor(3.0)
    .with_mobile(true)
    .with_touch(true);

// Builder form; anything left unset keeps its default.
let custom = Viewport::builder()
    .width(1440)
    .height(900)
    .device_scale_factor(2.0)
    .build();
```

## Tabs

For anything beyond a single capture — navigation, scripting, waiting on
content that appears late — work with a tab directly.

```rust
use cdp_html_shot::{Browser, CaptureOptions, Viewport};

let browser = Browser::new().await?;
let tab = browser.new_tab().await?;

tab.set_viewport(&Viewport::new(1280, 720).with_device_scale_factor(2.0))
    .await?;

tab.goto("https://example.com").await?;
println!("{}", tab.evaluate_as_string("document.title").await?);

// Wait for content rendered after load, then capture just that element.
let chart = tab.wait_for_selector(".chart", 5_000).await?;
let base64 = chart.screenshot_with_options(CaptureOptions::raw_png()).await?;

// Or the whole page.
let page = tab.screenshot(CaptureOptions::high_quality_jpeg()).await?;

tab.close().await?;
browser.close_async().await?;
```

`set_content` injects HTML without a navigation, which is what `capture_html`
uses internally.

## Launching the browser

### Choosing a browser

`Browser::new()` searches, in order: the `CHROME` environment variable, the
usual install locations for the platform, then `PATH`. Flatpak and Snap
wrappers are skipped, since they cannot be executed directly.

To be explicit, pass a path:

```rust
use cdp_html_shot::Browser;

let browser = Browser::new_with_path("/opt/chrome/chrome").await?;
```

### Launch options

`LaunchOptions` covers the rest: headed mode, the user agent, extra Chromium
switches, and where the throwaway profile is created.

```rust
use cdp_html_shot::{Browser, LaunchOptions};

let browser = Browser::launch_with(
    LaunchOptions::new()
        .headless(false)
        .user_agent("my-crawler/1.0")
        .arg("--proxy-server=socks5://127.0.0.1:1080")
        .args(["--lang=zh-CN", "--force-color-profile=srgb"]),
)
.await?;
```

Switches given to `arg` and `args` are appended **after** the built-in ones.
Chromium honours the last occurrence of a repeated switch, so anything set this
way overrides the corresponding default — including defaults this crate sets.

### About the default user agent

Headless Chromium reports `HeadlessChrome/<version>`, which plenty of sites
treat as a bot signal, so the default restates the same build as an ordinary
`Chrome/<version>`. The version is read from the executable rather than
hardcoded: a stale version string contradicts the client hints the browser
still reports truthfully through `navigator.userAgentData`, and that
contradiction is a louder signal than the `Headless` token ever was. If the
version cannot be read, no override is passed and the browser keeps its own.

### Shared instance

For long-running processes, `Browser::instance()` returns a process-wide
browser, relaunching it if it has died. `Browser::shutdown_global()` closes it.

```rust
use cdp_html_shot::Browser;

let browser = Browser::instance().await;
let base64 = browser.capture_html("<b id='x'>hi</b>", "#x").await?;

Browser::shutdown_global().await;
```

With the `atexit` feature, `ExitHook` also shuts it down on Ctrl-C.

## Platform notes

**Android (Termux)** — install the browser with `pkg install chromium`; it is
found automatically under `$PREFIX/bin`.

Android is the one platform where the GPU service is *not* folded into the
browser process. Under heavy WebGL work it crashes there and, sharing a process
with the browser, takes the CDP connection down with it — surfacing as an
unexplained transport error in the middle of a session. Android therefore
launches with `--disable-gpu --enable-unsafe-swiftshader`, which leaves WebGL
working through ANGLE's SwiftShader fallback. Pass `--in-process-gpu` through
`arg` to opt back in.

## Testing

Unit tests need no browser and run in milliseconds:

```text
cargo test
```

End-to-end tests drive a real browser and are ignored by default, so the
default run stays hermetic:

```text
cargo test --all-features -- --ignored
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
