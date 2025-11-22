use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use futures_util::future::join_all;
use std::path::Path;
use tokio::fs;

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en" style="height:100%">
<body style="background: #f4f4f4; display: flex; justify-content: center; align-items: center; margin: 0;">
    <div style="padding: 40px; background: white; border-radius: 10px; box-shadow: 0 4px 10px rgba(0,0,0,0.1); text-align: center;">
        <h1 style="color: #d32f2f; font-family: sans-serif;">Batch Capture</h1>
        <div style="font-size: 24px; color: #555;">ID: REPLACE_ME</div>
    </div>
</body>
</html>"#;

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;

    let output_dir = Path::new("screenshots");
    if !output_dir.exists() {
        fs::create_dir(output_dir).await?;
    }

    let count = 5;
    println!("Launching {} tabs concurrently...", count);

    // 1. 并发创建 Tabs
    let mut tab_futures = Vec::new();
    for _ in 0..count {
        tab_futures.push(browser.new_tab());
    }
    let tabs = futures::future::try_join_all(tab_futures).await?;

    // 2. 并发执行截图任务
    let mut screenshot_tasks = Vec::new();

    for (i, tab) in tabs.into_iter().enumerate() {
        let output_path = output_dir.join(format!("batch_{}.jpeg", i));
        let html_content = HTML_TEMPLATE.replace("REPLACE_ME", &format!("#{:03}", i));

        screenshot_tasks.push(tokio::spawn(async move {
            // 设置内容 -> 查找元素 -> 截图
            tab.set_content(&html_content).await?;
            let element = tab.find_element("body").await?;
            let base64 = element.screenshot().await?;

            // 立即关闭 Tab 释放内存
            tab.close().await?;

            // 解码并保存
            let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;
            fs::write(&output_path, img_data).await?;

            println!("Captured: {:?}", output_path);
            Ok::<(), anyhow::Error>(())
        }));
    }

    // 等待所有任务完成
    let results = join_all(screenshot_tasks).await;

    // 检查错误
    for (i, res) in results.into_iter().enumerate() {
        match res {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => eprintln!("Task {} logic failed: {}", i, e),
            Err(e) => eprintln!("Task {} panic: {}", i, e),
        }
    }

    println!("All done. Closing browser...");
    browser.close_async().await?;

    Ok(())
}
