//! Every knob that decides how a capture is encoded.
//!
//! Writes one file per variant into `screenshots/`, so the differences are
//! there to look at rather than described in a comment.
//!
//! ```text
//! cargo run --example capture_options
//! ```

use anyhow::Result;
use base64::Engine;
use cdp_html_shot::{Browser, CaptureOptions, ImageFormat, Viewport};
use std::path::Path;

const CARD: &str = r#"
<html lang="en"><body style="margin:0;background:transparent">
  <div id="card" style="width:360px;padding:32px;border-radius:12px;
                        background:#1e293b;color:#f8fafc;font-family:sans-serif">
    <h1 style="margin:0 0 8px;font-size:28px">Capture options</h1>
    <p style="margin:0;color:#94a3b8">format · quality · scale · background</p>
  </div>
</body></html>
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let out = Path::new("screenshots");
    std::fs::create_dir_all(out)?;

    let browser = Browser::new().await?;

    // The default: JPEG at the browser's own scale.
    save(
        out,
        "default.jpeg",
        &browser.capture_html(CARD, "#card").await?,
    )?;

    // PNG, keeping the page's transparent background.
    let transparent = CaptureOptions::raw_png().with_omit_background(true);
    save(
        out,
        "transparent.png",
        &browser
            .capture_html_with_options(CARD, "#card", transparent)
            .await?,
    )?;

    // JPEG at a quality the protocol accepts; anything above 100 is clamped.
    let lossy = CaptureOptions::new()
        .with_format(ImageFormat::Jpeg)
        .with_quality(40);
    save(
        out,
        "quality-40.jpeg",
        &browser
            .capture_html_with_options(CARD, "#card", lossy)
            .await?,
    )?;

    // A higher device scale factor renders more pixels for the same layout.
    save(
        out,
        "hidpi-2x.jpeg",
        &browser.capture_html_hidpi(CARD, "#card", 2.0).await?,
    )?;

    let ultra = CaptureOptions::raw_png()
        .with_viewport(Viewport::new(1200, 800).with_device_scale_factor(3.0));
    save(
        out,
        "hidpi-3x.png",
        &browser
            .capture_html_with_options(CARD, "#card", ultra)
            .await?,
    )?;

    browser.close_async().await
}

/// Captures come back base64-encoded, which is what CDP returns; decode to get
/// the image itself.
fn save(dir: &Path, name: &str, base64: &str) -> Result<()> {
    let bytes = base64::prelude::BASE64_STANDARD.decode(base64)?;
    let path = dir.join(name);
    std::fs::write(&path, &bytes)?;
    println!("{:<20} {:>7} bytes", name, bytes.len());
    Ok(())
}
