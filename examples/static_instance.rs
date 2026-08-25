//! One browser shared across a whole process.
//!
//! `Browser::instance()` hands every caller the same browser, launching it on
//! first use and relaunching it if it has died. That suits a long-running
//! service, where paying the startup cost per capture would dominate.
//!
//! ```text
//! cargo run --example static_instance --features atexit
//! ```

use cdp_html_shot::{Browser, ExitHook};
use futures_util::future::join_all;

const CARD: &str = r#"
<html lang="en"><body style="margin:0">
  <div id="card" style="width:280px;padding:24px;background:#0f172a;color:#e2e8f0;
                        font:20px sans-serif;text-align:center">worker __ID__</div>
</body></html>
"#;

#[tokio::main]
async fn main() {
    // Ctrl-C would otherwise leave the browser running: the process dies
    // before any `Drop` gets to shut it down.
    let hook = ExitHook::new(|| {
        println!("\n[ExitHook] shutting the shared browser down...");
        futures::executor::block_on(Browser::shutdown_global());
        println!("[ExitHook] done");
    });
    if let Err(e) = hook.register() {
        eprintln!("could not register the exit hook: {e}");
    }

    // Concurrent work, all of it against the one browser. Only the first of
    // these launches anything.
    let work = (0..5).map(|i| {
        tokio::spawn(async move {
            let browser = Browser::instance().await;
            let html = CARD.replace("__ID__", &i.to_string());

            match browser.capture_html(&html, "#card").await {
                Ok(shot) => println!("worker {i}: captured {} base64 chars", shot.len()),
                Err(e) => eprintln!("worker {i}: {e}"),
            }
        })
    });

    join_all(work).await;

    // `shutdown_global` closes the shared browser. Going through `instance()`
    // here would be a mistake: it relaunches a browser that has already died,
    // only to close it again.
    println!("all workers done, shutting down");
    Browser::shutdown_global().await;
}
