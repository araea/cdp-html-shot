# cdp-html-shot

Rust 库：通过 Chrome DevTools Protocol 把 HTML 或网页元素截图为 PNG、JPEG 或 WebP。

## 安装

```toml
[dependencies]
cdp-html-shot = "0.2"
```

运行时需要 Chrome、Chromium 或 Edge。`Browser::new()` 会自动查找，也可用 `Browser::new_with_path()` 指定可执行文件。

## 快速开始

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

截图结果是 Base64 字符串，需自行解码为图像字节。

## 常用接口

- `capture_html`：注入 HTML 并截取指定选择器
- `capture_html_with_options`：用 `CaptureOptions` 设置格式、质量、透明背景、全页截图与视口
- `capture_html_hidpi`：按指定 `deviceScaleFactor` 输出
- `new_tab`：在同一浏览器中导航、执行脚本、等待选择器并截图
- `Browser::instance()`：进程级共享实例，结束时调用 `shutdown_global()`

浏览器启动参数使用 `LaunchOptions`，自定义参数通过 `arg` 或 `args` 传入。

## 平台与测试

Termux 可执行 `pkg install chromium`。Android 下默认关闭 GPU 并使用 SwiftShader，需要时可通过启动参数覆盖。

```sh
cargo test
cargo test --all-features -- --ignored
```

第二条命令需要本机浏览器，端到端测试默认被忽略。

## 许可证

可按 [Apache-2.0](LICENSE-APACHE) 或 [MIT](LICENSE-MIT) 使用。
