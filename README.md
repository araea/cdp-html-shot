# cdp-html-shot

Rust 库：通过 Chrome DevTools Protocol 把 HTML 或网页元素截图为 PNG、JPEG 或 WebP。

## 安装

```toml
[dependencies]
cdp-html-shot = "0.3"
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

浏览器启动参数使用 `LaunchOptions`。自定义参数通过 `arg` 或 `args` 传入。

默认关闭 Chromium 的端上优化模型下载（`--disable-features=OptimizationGuideModelDownloading`）。临时 profile 是一次性的，这个模型每个新 profile 都要重下约 2.8 GB。调用方自带 `--disable-features` 时，这一项会并进调用方的列表，不会被顶掉。

## 临时 profile 的生命周期

每次启动浏览器都会在系统临时目录下建一个 `cdp-shot_<时间>_<随机>` 目录作为 `--user-data-dir`，正常析构时删除。

目录里有一个 `.cdp-html-shot.lock`，由创建它的进程持有 `flock`。进程无论怎么死，内核都会释放这把锁；下一次启动时库扫同目录下的 `cdp-shot_*`，把锁已经没人持有的（也就是宿主被 SIGKILL、panic-abort、走 `std::process::exit()` 这类没走到析构的）删掉。所以不正常的退出不会永久留下 profile。

浏览器进程本身也随调用方退出：Unix 下启动时设 `PR_SET_PDEATHSIG`，调用方一死内核就杀掉浏览器，不留占着 `--user-data-dir` 的孤儿。

这把锁只在 Unix 上真正生效。Windows 下目录仍由析构删除，但没有这种自愈清扫。

## 平台与测试

Termux 可执行 `pkg install chromium`。Android 下默认关闭 GPU 并使用 SwiftShader。需要时可通过启动参数覆盖。

```sh
cargo test
cargo test --all-features -- --ignored
```

第二条命令需要本机浏览器。端到端测试默认被忽略。

## 许可证

可按 [Apache-2.0](LICENSE-APACHE) 或 [MIT](LICENSE-MIT) 使用。
