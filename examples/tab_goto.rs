//! Driving a tab: navigate, wait for content, script it, capture part of it.
//!
//! ```text
//! cargo run --example tab_goto
//! ```

use anyhow::Result;
use base64::Engine;
use cdp_html_shot::{Browser, CaptureOptions};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let out = Path::new("screenshots");
    std::fs::create_dir_all(out)?;

    let browser = Browser::new().await?;
    let tab = browser.new_tab().await?;

    println!("Navigating to rust-lang.org...");
    tab.goto("https://www.rust-lang.org/").await?;

    // `goto` returns once the load event fires, but content rendered after it
    // is not there yet. Wait for the element instead of sleeping for a guessed
    // duration: it returns the moment the element appears, and fails loudly if
    // it never does.
    let main = tab.wait_for_selector("main", 10_000).await?;

    println!("title:  {}", tab.title().await?);
    println!("url:    {}", tab.url().await?);

    let headings = tab
        .evaluate_as_string("document.querySelectorAll('h1, h2').length")
        .await?;
    println!("h1/h2:  {headings}");

    // Just the hero, then the whole page for comparison.
    write(
        out.join("main.png"),
        &main
            .screenshot_with_options(CaptureOptions::raw_png())
            .await?,
    )?;
    write(
        out.join("page.jpeg"),
        &tab.screenshot(CaptureOptions::high_quality_jpeg()).await?,
    )?;

    tab.close().await?;
    browser.close_async().await
}

fn write(path: std::path::PathBuf, base64: &str) -> Result<()> {
    let bytes = base64::prelude::BASE64_STANDARD.decode(base64)?;
    std::fs::write(&path, &bytes)?;
    println!("wrote   {} ({} bytes)", path.display(), bytes.len());
    Ok(())
}
