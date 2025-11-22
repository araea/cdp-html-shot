use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use shindan_maker::{ShindanClient, ShindanDomain};
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let shindan_ids = [
        // ("1150687", "抽老婆"),
        ("1252750", "人设生成"),
        // ("1222992", "Fantasy Stats"),
        ("1253379", "一封给新生魔女的建议信"),
        // ("1253257", "你会拥有怎样的异世界旅程？"),
        // ("1150916", "哥们考试"),
        // ("1252510", "图表组件"),
    ];

    let client = ShindanClient::new(ShindanDomain::Jp)?;
    let browser = Browser::new().await?;
    // let browser = Browser::new_with_head().await?;

    for (shindan_id, desc) in shindan_ids.iter() {
        let output_file = format!("shindan_{}.jpeg", shindan_id);

        if Path::new(&output_file).exists() {
            println!(
                "Warning: {} already exists, will be overwritten",
                output_file
            );
        }

        let (html_str, title) = client.get_html_str_with_title(shindan_id, "ARuFa").await?;

        println!("Result for [{} - {}]: {}", shindan_id, desc, title);

        let base64 = browser.capture_html(&html_str, "#title_and_result").await?;
        let img_data = base64::prelude::BASE64_STANDARD.decode(base64)?;

        fs::write(&output_file, img_data)?;
        println!("Screenshot saved to {}", output_file);
    }

    Ok(())
}
