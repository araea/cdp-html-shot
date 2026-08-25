//! Many captures at once, from a single browser.
//!
//! Tabs are cheap; browsers are not. Opening a tab per job and closing it as
//! soon as its image is out keeps memory flat while the work runs in parallel.
//!
//! ```text
//! cargo run --example take_shots
//! ```

use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use futures_util::future::join_all;
use std::path::Path;

const HTML: &str = r#"
<html lang="en"><body style="margin:0;display:flex;justify-content:center;
                             align-items:center;height:100vh;background:#f4f4f4">
  <div style="padding:40px;background:#fff;border-radius:10px;text-align:center;
              box-shadow:0 4px 10px rgba(0,0,0,.1);font-family:sans-serif">
    <h1 style="margin:0;color:#d32f2f">Batch capture</h1>
    <div style="font-size:24px;color:#555">ID: __ID__</div>
  </div>
</body></html>
"#;

#[tokio::main]
async fn main() -> Result<()> {
    const COUNT: usize = 5;

    let out = Path::new("screenshots");
    std::fs::create_dir_all(out)?;

    let browser = Browser::new().await?;
    println!("capturing {COUNT} images concurrently...");

    let jobs = (0..COUNT).map(|i| {
        let browser = browser.clone();
        let path = out.join(format!("batch_{i}.jpeg"));

        tokio::spawn(async move {
            let tab = browser.new_tab().await?;
            tab.set_content(&HTML.replace("__ID__", &format!("{i:03}")))
                .await?;

            let element = tab.find_element("body").await?;
            let base64 = element.screenshot().await?;

            // Close the tab before decoding: the image is already in hand, and
            // the tab is the expensive part to keep around.
            tab.close().await?;

            let bytes = base64::prelude::BASE64_STANDARD.decode(base64)?;
            std::fs::write(&path, &bytes)?;
            println!("  {} ({} bytes)", path.display(), bytes.len());

            Ok::<(), anyhow::Error>(())
        })
    });

    for (i, outcome) in join_all(jobs).await.into_iter().enumerate() {
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("job {i} failed: {e}"),
            Err(e) => eprintln!("job {i} panicked: {e}"),
        }
    }

    browser.close_async().await
}
