use cdp_html_shot::{Browser, CaptureOptions, Viewport};
use std::path::PathBuf;
use tokio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let custom_browser_path = detect_browser_path().expect("未找到浏览器路径，请手动指定");
    println!("使用浏览器路径: {:?}", custom_browser_path);

    // ==========================================
    // 场景 1: 创建一个独立的 Browser 实例 (带自定义路径)
    // ==========================================
    println!("\n--- 场景 1: 独立实例 ---");
    {
        println!("正在启动独立浏览器实例...");
        let browser = Browser::new_with_path(&custom_browser_path).await?;

        let html = r#"
            <html>
                <body style="background: linear-gradient(to right, #ff7e5f, #feb47b); height: 100vh; display: flex; justify-content: center; align-items: center;">
                    <h1 id="target" style="color: white; font-family: sans-serif; font-size: 4rem; text-shadow: 2px 2px 4px rgba(0,0,0,0.3);">
                        独立实例截图
                    </h1>
                </body>
            </html>
        "#;

        let base64_img = browser.capture_html(html, "#target").await?;
        println!("独立实例截图成功! Base64 长度: {}", base64_img.len());

        // 显式关闭 (或者等待它离开作用域自动关闭)
        browser.close_async().await?;
        println!("独立实例已关闭");
    }

    // ==========================================
    // 场景 2: 使用全局单例 (带自定义路径)
    // ==========================================
    println!("\n--- 场景 2: 全局单例 ---");

    // 第一次调用 instance_with_path 会初始化全局单例
    // 注意：如果全局实例已经存在（比如之前调用过），这里的路径参数会被忽略
    println!("正在初始化全局浏览器实例...");
    let global_browser = Browser::instance_with_path(&custom_browser_path).await;

    let html_global = r#"
        <html>
            <body style="background: #333; color: #0f0; height: 100vh; display: flex; justify-content: center; align-items: center;">
                <div id="code" style="border: 2px solid #0f0; padding: 20px; font-family: monospace;">
                    全局单例工作正常
                </div>
            </body>
        </html>
    "#;

    // 高级用法：使用 Tab 手动操作，设置高分屏
    let tab = global_browser.new_tab().await?;

    // 设置视口
    tab.set_viewport(&Viewport::default().with_device_scale_factor(2.0))
        .await?;

    // 加载内容
    tab.set_content(html_global).await?;

    // 查找元素
    let element = tab.find_element("#code").await?;

    // 截图 (PNG)
    let png_data = element
        .screenshot_with_options(CaptureOptions::raw_png())
        .await?;
    println!("全局实例截图成功! Base64 长度: {}", png_data.len());

    // 关闭 Tab (浏览器进程仍然保留，供下次复用)
    tab.close().await?;

    // 在程序结束时，清理全局浏览器
    println!("正在关闭全局浏览器...");
    Browser::shutdown_global().await;
    println!("完成!");

    Ok(())
}

/// 简单的辅助函数，用于在示例中查找可能存在的浏览器路径
fn detect_browser_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    ];

    #[cfg(target_os = "macos")]
    let paths = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ];

    #[cfg(target_os = "linux")]
    let paths = [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];

    for path in paths.iter() {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    None
}
