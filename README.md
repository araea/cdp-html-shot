# cdp-html-shot

Rust 库，通过 Chrome DevTools Protocol 将 HTML 或网页元素截图为 PNG、JPEG 或 WebP。

## 安装

```toml
[dependencies]
cdp-html-shot = "0.3"
```

运行时需要 Chrome、Chromium 或 Edge。`Browser::new()` 自动查找浏览器；也可用 `Browser::new_with_path()` 指定路径。

## 示例

```rust
use anyhow::Result;
use base64::Engine;
use cdp_html_shot::Browser;

#[tokio::main]
async fn main() -> Result<()> {
    let browser = Browser::new().await?;
    let encoded = browser.capture_html("<h1 id=\"title\">Hello</h1>", "#title").await?;
    let image = base64::prelude::BASE64_STANDARD.decode(encoded)?;
    std::fs::write("screenshot.jpeg", image)?;
    browser.close_async().await
}
```

截图方法返回 Base64 字符串，调用方负责解码为图像字节。

## 接口

- `capture_html`：渲染 HTML 并截取指定选择器。
- `capture_html_with_options`：通过 `CaptureOptions` 设置格式、质量、透明背景、全页截图和视口。
- `capture_html_hidpi`：设置 `deviceScaleFactor` 截图。
- `new_tab`：在同一浏览器中导航、执行脚本、等待选择器和截图。
- `Browser::instance()`：获取进程级共享实例；结束时调用 `shutdown_global()`。

浏览器通过 `LaunchOptions` 配置，可用 `arg` 或 `args` 添加启动参数。库默认关闭 Chromium 的端侧优化模型下载（`--disable-features=OptimizationGuideModelDownloading`），以免一次性临时 profile 重复下载约 2.8 GB 的模型；调用方提供的 `--disable-features` 会与该选项合并。

## 运行与清理

每次启动都会在系统临时目录创建独立的 `cdp-shot_*` profile，正常析构时删除。Unix 下进程锁用于清理异常退出后遗留的 profile，浏览器也会随宿主进程退出。Windows 仅在正常析构时清理目录。

Termux 可通过 `pkg install chromium` 安装浏览器。Android 默认关闭 GPU 并使用 SwiftShader，必要时可用启动参数覆盖。

## 测试

```sh
cargo test
cargo test --all-features -- --ignored
```

第二条命令需要本机浏览器；端到端测试默认忽略。

## 许可证

可按 [Apache-2.0](LICENSE-APACHE) 或 [MIT](LICENSE-MIT) 使用。
