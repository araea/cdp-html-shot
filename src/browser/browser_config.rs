use anyhow::{Context, Result, anyhow};
use rand::prelude::SliceRandom;
use std::net;
use std::path::{Path, PathBuf};
use which::which;

#[cfg(windows)]
use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

use crate::browser::temp_dir::CustomTempDir;

static DEFAULT_ARGS: [&str; 28] = [
    // === 核心模式 (必须用 new 才能保证渲染正确) ===
    // "--headless=new",
    // === 进程与内存优化 (针对低配) ===
    "--no-sandbox",                        // 减少进程开销 (Docker/Root 下必须)
    "--no-zygote",                         // 禁用 Zygote 进程，节省内存
    "--in-process-gpu",                    // 将 GPU 模拟放在主进程，减少进程上下文切换
    "--disable-dev-shm-usage",             // 解决 Docker 内存溢出
    "--js-flags=--max-old-space-size=512", // 限制 JS 堆内存为 512MB，防止 OOM 导致服务器卡死
    "--disable-features=Translate,OptimizationHints,MediaRouter,DialMediaRouteProvider", // 禁用无用后台特性
    "--disable-background-networking", // 禁止后台网络活动
    "--disable-component-update",      // 禁止组件更新
    "--disable-domain-reliability",    // 禁止域名可靠性监测
    // === 渲染优化 (软件渲染加速) ===
    "--disable-gpu",                 // 服务器通常无 GPU
    "--use-gl=swiftshader",          // 强制使用 CPU 软件渲染
    "--disable-software-rasterizer", // 注意：这里要删掉这行！不能禁用软件光栅化，否则白屏
    // "--disable-software-rasterizer",
    "--force-color-profile=srgb", // 避免颜色转换的 CPU 开销
    // === 网络与缓存 (提速关键) ===
    // 开启磁盘缓存，第二次生成相同图片会秒开
    "--disk-cache-dir=/tmp/chrome-cache",
    "--disk-cache-size=33554432", // 限制缓存 32MB，避免磁盘 I/O 爆炸
    "--enable-async-dns",         // 异步 DNS
    "--no-pings",                 // 禁止审计 Ping
    "--disable-ipv6",             // 如果你服务器不支持 IPv6，禁用它可以减少连接尝试超时
    // === 视觉与窗口 ===
    "--hide-scrollbars",       // 隐藏滚动条，不需要渲染它
    "--mute-audio",            // 静音
    "--window-size=1200,1600", // 按需设置，不要设太大 (如 4K)，像素越多 CPU 渲染越慢！
    // === 杂项 ===
    "--disable-breakpad",
    "--disable-infobars",
    "--disable-notifications",
    "--disable-popup-blocking",
    "--no-first-run",
    "--no-default-browser-check",
    // === 反爬伪装 ===
    "--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
];

pub(crate) struct BrowserConfig {
    debug_port: u16,
    pub(crate) headless: bool,
    pub(crate) temp_dir: CustomTempDir,
    pub(crate) executable_path: PathBuf,
}

impl BrowserConfig {
    pub(crate) fn new() -> Result<Self> {
        let temp_dir = std::env::current_dir()?.join("temp");

        Ok(Self {
            headless: true,
            executable_path: default_executable()?,
            debug_port: get_available_port().context("Failed to get available port")?,
            temp_dir: CustomTempDir::new(temp_dir, "cdp-html-shot")
                .context("Failed to create custom temporary directory")?,
        })
    }

    pub(crate) fn get_browser_args(&self) -> Vec<String> {
        let mut args = vec![
            format!("--remote-debugging-port={}", self.debug_port),
            format!("--user-data-dir={}", self.temp_dir.path().display()),
        ];

        args.extend(DEFAULT_ARGS.iter().map(|s| s.to_string()));
        if self.headless {
            args.push("--headless".to_string());
        }

        args
    }
}

fn default_executable() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("CHROME")
        && Path::new(&path).exists()
    {
        return Ok(path.into());
    }

    let apps = [
        "google-chrome-stable",
        "google-chrome-beta",
        "google-chrome-dev",
        "google-chrome-unstable",
        "chromium",
        "chromium-browser",
        "microsoft-edge-stable",
        "microsoft-edge-beta",
        "microsoft-edge-dev",
        "chrome",
        "chrome-browser",
        "msedge",
        "microsoft-edge",
    ];
    for app in apps {
        if let Ok(path) = which(app) {
            return Ok(path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let macos_apps = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta",
            "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/Applications/Microsoft Edge Beta.app/Contents/MacOS/Microsoft Edge Beta",
            "/Applications/Microsoft Edge Dev.app/Contents/MacOS/Microsoft Edge Dev",
            "/Applications/Microsoft Edge Canary.app/Contents/MacOS/Microsoft Edge Canary",
        ];
        for path in macos_apps.iter() {
            let path = Path::new(path);
            if path.exists() {
                return Ok(path.into());
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(path) = get_chrome_path_from_registry().filter(|p| p.exists()) {
            return Ok(path);
        }

        let windows_apps = [r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"];
        for path in windows_apps.iter() {
            let path = Path::new(path);
            if path.exists() {
                return Ok(path.into());
            }
        }
    }

    Err(anyhow!("Could not auto detect a chrome executable"))
}

#[cfg(windows)]
fn get_chrome_path_from_registry() -> Option<PathBuf> {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\chrome.exe")
        .and_then(|key| key.get_value::<String, _>(""))
        .map(PathBuf::from)
        .ok()
}

fn get_available_port() -> Option<u16> {
    let mut ports: Vec<u16> = (8000..9000).collect();
    ports.shuffle(&mut rand::thread_rng());
    ports.iter().find(|port| port_is_available(**port)).copied()
}

fn port_is_available(port: u16) -> bool {
    net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}
