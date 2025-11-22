use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use shindan_maker::{ShindanClient, ShindanDomain};
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let shindan_ids = [
        ("1150687", "抽老婆"),
        ("1252750", "人设生成"),
        ("1222992", "Fantasy Stats"),
    ];

    let client = ShindanClient::new(ShindanDomain::Jp)?;

    // 建议默认使用 headless，需要调试时再换成 new_with_head
    let browser = Browser::new().await?;
    // let browser = Browser::new_with_head().await?;

    // 确保输出目录存在
    let output_dir = Path::new("shindan_results");
    if !output_dir.exists() {
        fs::create_dir(output_dir)?;
    }

    for (shindan_id, desc) in shindan_ids.iter() {
        let output_file = output_dir.join(format!("shindan_{}.jpeg", shindan_id));

        let (html_str, title) = client.get_html_str_with_title(shindan_id, "ARuFa").await?;
        println!("Result for [{} - {}]: {}", shindan_id, desc, title);

        // 截图
        let base64 = browser.capture_html(&html_str, "#title_and_result").await?;
        let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;

        fs::write(&output_file, img_data)?;
        println!("Screenshot saved to {:?}", output_file);
    }

    // 显式关闭以清理 Temp 目录和僵尸进程
    browser.close_async().await?;
    Ok(())
}
