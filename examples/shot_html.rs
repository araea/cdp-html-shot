use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;

    const HTML: &str = r#"
        <html lang="en-US">
        <head>
            <style>
                body { background-color: #f0f0f0; font-family: sans-serif; padding: 20px; }
                .card { background: white; padding: 40px; border-radius: 8px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); }
                h1 { color: #333; margin-top: 0; }
            </style>
        </head>
        <body>
            <div class="card">
                <h1>My Test Page</h1>
                <p>Hello from Rust CDP Shot!</p>
            </div>
        </body>
        </html>
    "#;

    println!("Capturing HTML...");
    // 直接使用 capture_html 快捷方法
    let base64 = browser.capture_html(HTML, ".card").await?;
    let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;

    let dir = std::env::current_dir()?.join("screenshots");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    let output_path = dir.join("simple_shot.jpeg");
    fs::write(&output_path, img_data)?;

    println!("Screenshot saved to {:?}", output_path);

    // 退出前清理资源
    browser.close_async().await?;

    Ok(())
}
