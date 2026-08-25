//! Controlling how the browser itself is launched.
//!
//! `Browser::new()` finds a browser and picks sane switches. `LaunchOptions`
//! is for when those defaults do not fit — a browser somewhere else, a
//! particular user agent, or extra Chromium switches.
//!
//! ```text
//! cargo run --example launch_options
//! ```

use anyhow::Result;
use cdp_html_shot::{Browser, LaunchOptions};

#[tokio::main]
async fn main() -> Result<()> {
    // What the defaults produce. The user agent is not the browser's own:
    // headless Chromium reports `HeadlessChrome/<version>`, and the default
    // restates that same build as an ordinary `Chrome/<version>`.
    report("defaults", Browser::new().await?).await?;

    // A user agent of your own choosing.
    report(
        "user_agent",
        Browser::launch_with(LaunchOptions::new().user_agent("my-crawler/1.0")).await?,
    )
    .await?;

    // Arbitrary switches. These are appended after the built-in ones, and
    // Chromium honours the last occurrence of a repeated switch — so `arg`
    // overrides any default, including the user agent set just above.
    report(
        "arg overrides",
        Browser::launch_with(
            LaunchOptions::new()
                .user_agent("this-one-loses/1.0")
                .arg("--user-agent=this-one-wins/2.0")
                .args(["--lang=en-GB", "--force-color-profile=srgb"]),
        )
        .await?,
    )
    .await?;

    // A browser outside the search path. Set CHROME to try this arm; the
    // library looks that variable up on its own, so passing the path
    // explicitly is only needed when you want to bypass detection entirely.
    if let Ok(path) = std::env::var("CHROME") {
        report("explicit path", Browser::new_with_path(&path).await?).await?;
    } else {
        println!("{:<16} skipped (set CHROME to try it)", "explicit path");
    }

    Ok(())
}

/// Asks the browser what it thinks its own user agent is — the only witness
/// that a switch actually reached it.
async fn report(label: &str, browser: Browser) -> Result<()> {
    let tab = browser.new_tab().await?;
    println!(
        "{label:<16} {}",
        tab.evaluate_as_string("navigator.userAgent").await?
    );

    tab.close().await?;
    browser.close_async().await
}
