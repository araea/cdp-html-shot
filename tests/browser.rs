//! End-to-end tests that drive a real browser.
//!
//! They need a Chrome/Chromium installation, so they are ignored by default —
//! `cargo test` stays fast and hermetic. Run them with:
//!
//! ```text
//! cargo test --all-features -- --ignored
//! ```

use anyhow::Result;
use base64::Engine;
use cdp_html_shot::{Browser, CaptureOptions, ImageFormat, LaunchOptions, Viewport};

const CARD: &str = r#"
    <html lang="en"><body style="margin:0">
      <div id="card" style="width:320px;height:180px;background:#1e293b;color:#f8fafc;
                            font:20px/180px sans-serif;text-align:center">
        cdp-html-shot
      </div>
    </body></html>
"#;

fn decode(base64: &str) -> Vec<u8> {
    base64::prelude::BASE64_STANDARD
        .decode(base64)
        .expect("capture returned invalid base64")
}

/// Screenshots are returned as base64, so the only way to know the format
/// actually took effect is to look at the decoded bytes.
fn assert_jpeg(bytes: &[u8]) {
    assert!(
        bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "expected a JPEG, got {:02X?}",
        &bytes[..bytes.len().min(8)]
    );
}

fn assert_png(bytes: &[u8]) {
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "expected a PNG, got {:02X?}",
        &bytes[..bytes.len().min(8)]
    );
}

#[tokio::test]
#[ignore = "requires a Chrome/Chromium installation"]
async fn captures_an_element_as_jpeg() -> Result<()> {
    let browser = Browser::new().await?;
    let bytes = decode(&browser.capture_html(CARD, "#card").await?);

    assert_jpeg(&bytes);
    assert!(
        bytes.len() > 512,
        "image looks empty: {} bytes",
        bytes.len()
    );

    browser.close_async().await
}

#[tokio::test]
#[ignore = "requires a Chrome/Chromium installation"]
async fn capture_options_select_the_encoding() -> Result<()> {
    let browser = Browser::new().await?;

    let png = decode(
        &browser
            .capture_html_with_options(CARD, "#card", CaptureOptions::raw_png())
            .await?,
    );
    assert_png(&png);

    let jpeg = decode(
        &browser
            .capture_html_with_options(
                CARD,
                "#card",
                CaptureOptions::new()
                    .with_format(ImageFormat::Jpeg)
                    .with_quality(90),
            )
            .await?,
    );
    assert_jpeg(&jpeg);

    browser.close_async().await
}

/// A higher device scale factor must produce more pixels, not just a flag that
/// goes nowhere.
#[tokio::test]
#[ignore = "requires a Chrome/Chromium installation"]
async fn hidpi_produces_a_denser_image() -> Result<()> {
    let browser = Browser::new().await?;

    let at = |scale: f64| {
        let options = CaptureOptions::raw_png()
            .with_viewport(Viewport::new(400, 300).with_device_scale_factor(scale));
        browser.capture_html_with_options(CARD, "#card", options)
    };

    let normal = decode(&at(1.0).await?).len();
    let dense = decode(&at(3.0).await?).len();

    assert!(
        dense > normal,
        "3x capture ({dense} bytes) should exceed 1x ({normal} bytes)"
    );

    browser.close_async().await
}

#[tokio::test]
#[ignore = "requires a Chrome/Chromium installation"]
async fn tabs_navigate_and_evaluate() -> Result<()> {
    let browser = Browser::new().await?;
    let tab = browser.new_tab().await?;

    tab.set_content(CARD).await?;
    assert_eq!(
        tab.evaluate_as_string("document.querySelector('#card').textContent.trim()")
            .await?,
        "cdp-html-shot"
    );

    let element = tab.wait_for_selector("#card", 5_000).await?;
    assert_jpeg(&decode(&element.screenshot().await?));

    tab.close().await?;
    browser.close_async().await
}

/// `LaunchOptions::user_agent` has to reach the browser, not just the argument
/// vector — the browser is the only witness that counts.
#[tokio::test]
#[ignore = "requires a Chrome/Chromium installation"]
async fn launch_options_reach_the_browser() -> Result<()> {
    const AGENT: &str = "cdp-html-shot-integration/1.0";

    let browser = Browser::launch_with(LaunchOptions::new().user_agent(AGENT)).await?;
    let tab = browser.new_tab().await?;

    assert_eq!(tab.evaluate_as_string("navigator.userAgent").await?, AGENT);

    tab.close().await?;
    browser.close_async().await
}

/// The documented override rule, checked where it actually matters: a switch
/// passed through `arg` must beat the built-in of the same name.
#[tokio::test]
#[ignore = "requires a Chrome/Chromium installation"]
async fn caller_switches_override_the_defaults() -> Result<()> {
    const AGENT: &str = "overridden-by-arg/2.0";

    let browser = Browser::launch_with(
        LaunchOptions::new()
            .user_agent("set-by-user-agent/1.0")
            .arg(format!("--user-agent={AGENT}")),
    )
    .await?;
    let tab = browser.new_tab().await?;

    assert_eq!(tab.evaluate_as_string("navigator.userAgent").await?, AGENT);

    tab.close().await?;
    browser.close_async().await
}

/// A selector that matches nothing must say so. CDP answers `DOM.querySelector`
/// with node id 0 instead of an error, and taking that at face value used to
/// surface as "Missing backendNodeId" — a message with no bearing on the
/// actual problem.
#[tokio::test]
#[ignore = "requires a Chrome/Chromium installation"]
async fn a_selector_that_matches_nothing_names_the_selector() -> Result<()> {
    let browser = Browser::new().await?;
    let tab = browser.new_tab().await?;
    tab.set_content(CARD).await?;

    let err = tab
        .find_element("#no-such-element")
        .await
        .expect_err("a missing element must be an error")
        .to_string();

    assert!(
        err.contains("#no-such-element"),
        "the error should name the selector, got: {err}"
    );

    tab.close().await?;
    browser.close_async().await
}
