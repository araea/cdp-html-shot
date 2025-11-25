use cdp_html_shot::{Browser, CaptureOptions, ImageFormat, Viewport};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let browser = Browser::new().await?;

    // 方式 1: 使用便捷方法获取 2x 高清截图
    let _shot = browser
        .capture_html_hidpi("<h1>Hello</h1>", "h1", 2.0)
        .await?;

    // 方式 2: 完全自定义配置
    let options = CaptureOptions::new()
        .with_format(ImageFormat::Png)
        .with_viewport(
            Viewport::new(1920, 1080).with_device_scale_factor(3.0), // 3x 超高清
        )
        .with_omit_background(true); // 透明背景

    let _shot = browser
        .capture_html_with_options(BATCH_CAPTURE_HTML, "body", options)
        .await?;

    // 方式 3: 手动控制 Tab
    let tab = browser.new_tab().await?;
    tab.set_viewport(&Viewport::new(1280, 720).with_device_scale_factor(2.0))
        .await?;
    tab.set_content(BATCH_CAPTURE_HTML).await?;
    // ...

    Ok(())
}

const BATCH_CAPTURE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en" style="height:100%">
<body style="background: #f4f4f4; display: flex; justify-content: center; align-items: center; margin: 0;">
    <div style="padding: 40px; background: white; border-radius: 10px; box-shadow: 0 4px 10px rgba(0,0,0,0.1); text-align: center;">
        <h1 style="color: #d32f2f; font-family: sans-serif;">Batch Capture</h1>
        <div style="font-size: 24px; color: #555;">ID: REPLACE_ME</div>
    </div>
</body>
</html>"#;
