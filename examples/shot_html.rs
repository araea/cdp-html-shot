use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new_with_head().await?;

    const HTML: &str = r#"
        <html lang="en-US">
        <body>
        <h1>My test page</h1>
        <p>Hello, Rust!</p>
        </body>
        </html>
    "#;

    let base64 = browser.capture_html(HTML, "html").await?;

    let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;

    let dir = std::env::current_dir()?.join("cache");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("test0.jpeg"), img_data)?;

    Ok(())
}
