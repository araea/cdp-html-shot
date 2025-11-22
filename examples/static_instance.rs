use cdp_html_shot::{Browser, ExitHook};
use futures_util::future::join_all;
use tokio::task;

#[tokio::main]
async fn main() {
    let hook = ExitHook::new(|| {
        println!("Performing cleanup...");
        Browser::close_instance();
        println!("Cleanup completed!");
    });

    hook.register().unwrap();

    println!("Application running... Press Ctrl+C to exit");

    let mut handles = Vec::new();

    for _ in 0..10 {
        let handle = task::spawn(async move {
            let browser = Browser::instance().await;
            let tab = browser.new_tab().await.unwrap();
            tab.close().await.unwrap();
        });
        handles.push(handle);
    }

    join_all(handles).await;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
