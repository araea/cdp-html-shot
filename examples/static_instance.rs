use cdp_html_shot::{Browser, ExitHook};
use futures_util::future::join_all;
use std::time::Duration;
use tokio::task;

#[tokio::main]
async fn main() {
    // 注册退出钩子 (Ctrl+C)
    let hook = ExitHook::new(|| {
        println!("\n[ExitHook] Cleaning up global browser instance...");
        // 使用 futures::executor::block_on 在同步上下文中运行 async 清理逻辑
        let _ = futures::executor::block_on(async {
            // 获取单例（这是一个 Arc 克隆）
            let browser = Browser::instance().await;
            // 关闭它（这会杀死底层进程并删除 Temp 目录）
            let _ = browser.close_async().await;
        });
        println!("[ExitHook] Cleanup completed!");
    });

    // 注册 hook，忽略错误（例如已注册过 handler）
    if let Err(e) = hook.register() {
        eprintln!("Failed to register exit hook: {}", e);
    }

    println!("Application running... utilizing Singleton Browser.");

    let mut handles = Vec::new();

    // 启动 5 个并发任务使用单例浏览器
    for i in 0..5 {
        let handle = task::spawn(async move {
            // 获取单例引用（开销很小，不会启动新浏览器）
            let browser = Browser::instance().await;
            println!("Task {} acquired browser instance", i);

            if let Ok(tab) = browser.new_tab().await {
                // 模拟业务逻辑
                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = tab.close().await;
            }
            println!("Task {} finished", i);
        });
        handles.push(handle);
    }

    join_all(handles).await;

    println!("All tasks done. Cleaning up...");

    // 正常退出时也主动关闭
    Browser::instance().await.close_async().await.unwrap();
}
