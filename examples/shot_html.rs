//! The shortest useful thing this crate does: HTML in, image file out.
//!
//! ```text
//! cargo run --example shot_html
//! ```

use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;
use std::path::Path;

const HTML: &str = r#"
<html lang="en">
  <head><style>
    body { margin: 0; background: #f1f5f9; font-family: sans-serif; padding: 24px; }
    .card { background: #fff; padding: 40px; border-radius: 8px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, .1); }
    h1 { margin: 0 0 8px; color: #0f172a; }
    p  { margin: 0; color: #64748b; }
  </style></head>
  <body>
    <div class="card">
      <h1>My Test Page</h1>
      <p>Hello from cdp-html-shot</p>
    </div>
  </body>
</html>
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;

    // The selector picks what ends up in the image: the card, not the page
    // around it.
    let base64 = browser.capture_html(HTML, ".card").await?;
    let bytes = base64::prelude::BASE64_STANDARD.decode(base64)?;

    let out = Path::new("screenshots");
    std::fs::create_dir_all(out)?;
    let path = out.join("simple_shot.jpeg");
    std::fs::write(&path, &bytes)?;

    println!("wrote {} ({} bytes)", path.display(), bytes.len());

    browser.close_async().await
}
