use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use tokio::time;

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;

    // Only in headless mode
    browser.close_init_tab().await?;

    let tab = browser.new_tab().await?;

    tab.goto("https://www.rust-lang.org/").await?;

    const HTML: &str = r#"
        <html lang="en-US">
        <body>
        <h1>My test page</h1>
        <p>Hello, Rust!</p>
        </body>
        </html>
    "#;

    tab.set_content(HTML).await?;

    let element = tab.find_element("html").await?;
    let base64 = element.screenshot().await?;
    tab.close().await?;

    let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;
    std::fs::write("test0.jpeg", img_data)?;

    time::sleep(time::Duration::from_secs(5)).await;
    Ok(())
}
