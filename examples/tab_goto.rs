use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use std::path::Path;
use tokio::{fs, time};

#[tokio::main]
async fn main() -> Result<()> {
    let output_dir = Path::new("screenshots");
    if !output_dir.exists() {
        fs::create_dir(output_dir).await?;
    }

    let browser = Browser::new().await?;

    println!("Navigating to rust-lang.org...");
    let tab = browser.new_tab().await?;

    // 导航到网页
    tab.goto("https://www.rust-lang.org/").await?;

    println!("Waiting for render...");
    // 等待页面渲染和资源加载 (简单实现使用 sleep)
    time::sleep(time::Duration::from_secs(2)).await;

    // 查找元素并截图
    let element = tab.find_element("body").await?;
    let base64 = element.screenshot().await?;
    let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;

    let output_path = output_dir.join("web_shot.jpeg");
    fs::write(&output_path, img_data).await?;
    println!("Saved {:?}", output_path);

    // 关闭 Tab
    tab.close().await?;

    // 关闭浏览器
    browser.close_async().await?;

    Ok(())
}
