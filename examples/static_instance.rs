use cdp_html_shot::{Browser, ExitHook};
use futures_util::future::join_all;
use std::time::Duration;
use tokio::task;

#[tokio::main]
async fn main() {
    let hook = ExitHook::new(|| {
        println!("\n[ExitHook] Cleaning up global browser instance...");
        let _ = futures::executor::block_on(async {
            let browser = Browser::instance().await;
            let _ = browser.close_async().await;
        });
        println!("[ExitHook] Cleanup completed!");
    });

    if let Err(e) = hook.register() {
        eprintln!("Failed to register exit hook: {}", e);
    }

    println!("Application running... utilizing Singleton Browser.");

    let mut handles = Vec::new();

    for i in 0..5 {
        let handle = task::spawn(async move {
            let browser = Browser::instance().await;
            println!("Task {} acquired browser instance", i);

            if let Ok(tab) = browser.new_tab().await {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = tab.close().await;
            }
            println!("Task {} finished", i);
        });
        handles.push(handle);
    }

    join_all(handles).await;

    println!("All tasks done. Cleaning up...");

    Browser::instance().await.close_async().await.unwrap();
}
