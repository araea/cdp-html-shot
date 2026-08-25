use crate::tab::Tab;
use crate::transport::{Transport, TransportResponse, next_id};
use crate::types::{CaptureOptions, Viewport};
use anyhow::{Context, Result, anyhow};
use rand::{RngExt, rng};
use regex::Regex;
use serde_json::json;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use which::which;

/// Temporary directory for browser user data, deleted on drop.
struct CustomTempDir {
    path: PathBuf,
}

impl CustomTempDir {
    fn new(base: PathBuf, prefix: &str) -> Result<Self> {
        std::fs::create_dir_all(&base)?;
        let name = format!(
            "{}_{}_{}",
            prefix,
            chrono::Local::now().format("%Y%m%d_%H%M%S"),
            rng()
                .sample_iter(&rand::distr::Alphanumeric)
                .take(6)
                .map(char::from)
                .collect::<String>()
        );
        let path = base.join(name);
        std::fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for CustomTempDir {
    fn drop(&mut self) {
        for i in 0..10 {
            if std::fs::remove_dir_all(&self.path).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100 * (i as u64 + 1).min(3)));
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct BrowserProcess {
    child: Child,
    _temp: CustomTempDir,
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Flags deciding where the GPU service lives.
///
/// Everywhere but Android the service is hosted inside the browser process,
/// which keeps the process count down. Android cannot use that: under any real
/// WebGL load the GPU service crashes there, and because it shares a process
/// with the browser it takes the CDP connection down with it — surfacing as an
/// unexplained transport error in the middle of a session. Disabling the GPU
/// instead leaves WebGL working through ANGLE's SwiftShader fallback, which
/// modern Chromium only enables when asked.
#[cfg(not(target_os = "android"))]
const GPU_ARGS: &[&str] = &["--in-process-gpu"];
#[cfg(target_os = "android")]
const GPU_ARGS: &[&str] = &["--disable-gpu", "--enable-unsafe-swiftshader"];

/// How the browser process is launched.
///
/// Every field has a working default, so this is only worth reaching for when
/// the defaults do not fit: a browser outside the search path, a particular
/// user agent, or extra Chromium switches.
///
/// Switches added with [`LaunchOptions::arg`] are appended *after* the
/// built-in ones. Chromium honours the last occurrence of a repeated switch,
/// so a caller can override any default this way.
///
/// ```no_run
/// # use cdp_html_shot::{Browser, LaunchOptions};
/// # async fn run() -> anyhow::Result<()> {
/// let browser = Browser::launch_with(
///     LaunchOptions::new()
///         .arg("--proxy-server=socks5://127.0.0.1:1080")
///         .user_agent("my-crawler/1.0"),
/// )
/// .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    headless: bool,
    path: Option<PathBuf>,
    user_agent: Option<String>,
    extra_args: Vec<String>,
    user_data_root: Option<PathBuf>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            headless: true,
            path: None,
            user_agent: None,
            extra_args: Vec::new(),
            user_data_root: None,
        }
    }
}

impl LaunchOptions {
    /// Headless, browser auto-detected, default user agent.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether to run headless. Defaults to `true`.
    pub fn headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    /// Use this executable instead of searching for one.
    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Override the user agent.
    ///
    /// Left unset, the browser's own version is restated without the
    /// `Headless` token — see the note on [`Browser::launch_with`].
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Append one extra Chromium switch, overriding any built-in of the same
    /// name.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Append several extra Chromium switches.
    pub fn args<I, T>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.extra_args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Directory to create the throwaway user-data directory under.
    ///
    /// Defaults to the system temp directory.
    pub fn user_data_root(mut self, root: impl AsRef<Path>) -> Self {
        self.user_data_root = Some(root.as_ref().to_path_buf());
        self
    }
}

#[derive(Clone)]
pub struct Browser {
    transport: Arc<Transport>,
    process: Arc<Mutex<Option<BrowserProcess>>>,
}

static GLOBAL_BROWSER: Mutex<Option<Browser>> = Mutex::const_new(None);

impl Browser {
    /// Launches a new headless browser instance using the default browser path.
    pub async fn new() -> Result<Self> {
        Self::launch_with(LaunchOptions::new()).await
    }

    /// Launches a new headless browser instance using a custom executable path.
    pub async fn new_with_path(path: impl AsRef<Path>) -> Result<Self> {
        Self::launch_with(LaunchOptions::new().path(path)).await
    }

    /// Launches a new browser instance with head visible using the default browser path.
    pub async fn new_with_head() -> Result<Self> {
        Self::launch_with(LaunchOptions::new().headless(false)).await
    }

    /// Launches a new browser instance with head visible using a custom executable path.
    pub async fn new_with_head_and_path(path: impl AsRef<Path>) -> Result<Self> {
        Self::launch_with(LaunchOptions::new().headless(false).path(path)).await
    }

    /// Launches a browser with explicit [`LaunchOptions`].
    ///
    /// Unless a user agent is set explicitly, the browser is launched with its
    /// own version restated without the `Headless` token: headless Chromium
    /// reports `HeadlessChrome/<version>`, which many sites treat as a bot
    /// signal. The version is read from the executable rather than hardcoded,
    /// because a stale version string contradicts the client hints the browser
    /// still reports truthfully through `navigator.userAgentData` — and that
    /// contradiction is a louder signal than the `Headless` token ever was. If
    /// the version cannot be read, no override is passed at all.
    pub async fn launch_with(options: LaunchOptions) -> Result<Self> {
        let root = match options.user_data_root.clone() {
            Some(root) => root,
            None => Self::user_data_root(),
        };
        let temp = CustomTempDir::new(root, "cdp-shot")?;
        let exe = Self::find_chrome(options.path.clone())?;
        let port = (8000..9000)
            .find(|&p| std::net::TcpListener::bind(("127.0.0.1", p)).is_ok())
            .ok_or(anyhow!("No available port"))?;

        let mut args = vec![
            format!("--remote-debugging-port={}", port),
            format!("--user-data-dir={}", temp.path.display()),
            "--no-sandbox".into(),
            "--no-zygote".into(),
            "--disable-dev-shm-usage".into(),
            "--disable-background-networking".into(),
            "--disable-default-apps".into(),
            "--disable-extensions".into(),
            "--disable-sync".into(),
            "--disable-translate".into(),
            "--metrics-recording-only".into(),
            "--safebrowsing-disable-auto-update".into(),
            "--mute-audio".into(),
            "--no-first-run".into(),
            "--hide-scrollbars".into(),
            "--window-size=1200,1600".into(),
        ];
        args.extend(GPU_ARGS.iter().map(|a| a.to_string()));

        if let Some(agent) = options
            .user_agent
            .clone()
            .or_else(|| Self::default_user_agent(&exe))
        {
            args.push(format!("--user-agent={agent}"));
        }
        if options.headless {
            args.push("--headless=new".into());
        }
        // Last, so a caller can override any of the above: Chromium honours
        // the last occurrence of a repeated switch.
        args.extend(options.extra_args.iter().cloned());

        #[cfg(windows)]
        let mut cmd = {
            use std::os::windows::process::CommandExt;
            let mut c = Command::new(&exe);
            c.creation_flags(0x08000000);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = Command::new(&exe);

        let mut child = cmd
            .args(args)
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn browser executable: {:?}", exe))?;

        let stderr = child.stderr.take().context("No stderr")?;
        let ws_url = Self::wait_for_ws(stderr).await?;

        Ok(Self {
            transport: Arc::new(Transport::new(&ws_url).await?),
            process: Arc::new(Mutex::new(Some(BrowserProcess { child, _temp: temp }))),
        })
    }

    /// Where the throwaway user-data directory is created.
    ///
    /// The system temp directory, so that a browser killed before its `Drop`
    /// runs leaves its profile somewhere the OS cleans up. Earlier versions
    /// used `./temp`, which littered whatever project the caller happened to
    /// run from and failed outright when that directory was read-only. The
    /// working directory is still the fallback, for the rare system with no
    /// usable temp directory.
    fn user_data_root() -> PathBuf {
        let tmp = std::env::temp_dir();
        if tmp.is_dir() {
            return tmp;
        }
        std::env::current_dir()
            .map(|cwd| cwd.join("temp"))
            .unwrap_or(tmp)
    }

    /// The browser's own version, restated as an ordinary desktop user agent.
    ///
    /// `None` when the version cannot be read, in which case the caller passes
    /// no `--user-agent` at all and the browser keeps its own.
    fn default_user_agent(exe: &Path) -> Option<String> {
        let output = Command::new(exe).arg("--version").output().ok()?;
        // "Chromium 149.0.7827.155", "Google Chrome 131.0.6778.86", ...
        let major = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .find_map(|token| {
                let mut parts = token.split('.');
                let major = parts.next()?.parse::<u32>().ok()?;
                // A version is major.minor.build.patch; anything else is noise.
                (parts.count() == 3).then_some(major)
            })?;

        // Android runs the ordinary Linux build under Termux, and its client
        // hints say so, so it takes the Linux token too.
        let platform = if cfg!(target_os = "windows") {
            "Windows NT 10.0; Win64; x64"
        } else if cfg!(target_os = "macos") {
            "Macintosh; Intel Mac OS X 10_15_7"
        } else {
            "X11; Linux x86_64"
        };

        Some(format!(
            "Mozilla/5.0 ({platform}) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/{major}.0.0.0 Safari/537.36"
        ))
    }

    /// Attempts to locate a Chrome or Edge executable in the system.
    fn find_chrome(custom_path: Option<PathBuf>) -> Result<PathBuf> {
        // 1. Try custom path if provided
        if let Some(path) = custom_path {
            if path.exists() {
                return Ok(path);
            }
            return Err(anyhow!("Custom browser path does not exist: {:?}", path));
        }

        // 2. Try environment variable
        if let Ok(p) = std::env::var("CHROME") {
            let p = PathBuf::from(p);
            if p.exists() {
                return Ok(p);
            }
        }

        // 3. Try platform specific paths
        #[cfg(target_os = "windows")]
        {
            let paths = [
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            ];
            for p in paths {
                if Path::new(p).exists() {
                    return Ok(p.into());
                }
            }

            use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};
            let keys = [
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
            ];
            for k in keys {
                if let Ok(rk) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(k)
                    && let Ok(v) = rk.get_value::<String, _>("")
                {
                    if Path::new(&v).exists() {
                        return Ok(v.into());
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let paths = [
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            ];
            for p in paths {
                if Path::new(p).exists() {
                    return Ok(p.into());
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            let paths = [
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
            ];
            for p in paths {
                if Path::new(p).exists() {
                    return Ok(p.into());
                }
            }
        }

        // Android is its own `target_os`, so the Linux arm above never runs
        // there. Under Termux the browser lives in `$PREFIX/bin`, whose
        // location differs between the main and F-Droid builds — hence
        // consulting the variable rather than only the usual path.
        #[cfg(target_os = "android")]
        {
            let prefix = std::env::var("PREFIX")
                .unwrap_or_else(|_| "/data/data/com.termux/files/usr".into());
            for name in ["chromium-browser", "chromium", "chrome"] {
                let p = Path::new(&prefix).join("bin").join(name);
                if p.exists() {
                    return Ok(p);
                }
            }
        }

        // 4. Try common commands using `which`
        let apps = [
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "chrome",
            "msedge",
            "microsoft-edge",
        ];
        for app in apps {
            if let Ok(p) = which(app) {
                let p_str = p.to_string_lossy();
                // Check direct path for obvious flatpak/snap markers
                if p_str.contains("/var/lib/flatpak") || p_str.contains("/snap/") {
                    continue;
                }

                // Check resolved path (in case of symlinks like /usr/bin/msedge -> /var/lib/flatpak/...)
                // Flatpak often installs a symlink in /usr/bin that points to the internal flatpak data directory.
                // We must filter these because they cannot be executed directly without `flatpak run`.
                if let Ok(resolved) = std::fs::canonicalize(&p) {
                    let r_str = resolved.to_string_lossy();
                    if r_str.contains("/var/lib/flatpak") || r_str.contains("/snap/") {
                        continue;
                    }
                }

                return Ok(p);
            }
        }

        Err(anyhow!(
            "Chrome/Edge not found. Set CHROME env var or use new_with_path."
        ))
    }

    async fn wait_for_ws(stderr: std::process::ChildStderr) -> Result<String> {
        let (tx, rx) = oneshot::channel();

        // Spawn a blocking task to read stderr.
        // Important: We loop until the stream ends (process exit) to drain stderr,
        // preventing the pipe from filling up or closing prematurely which could kill the browser.
        tokio::task::spawn_blocking(move || {
            let reader = BufReader::new(stderr);
            let re =
                Regex::new(r"listening on (.*/devtools/browser/.*)\s*$").expect("Invalid regex");
            let mut found = false;
            let mut tx = Some(tx);

            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if !found && let Some(cap) = re.captures(&l) {
                            if let Some(tx) = tx.take() {
                                let _ = tx.send(Ok(cap[1].to_string()));
                            }
                            found = true;
                        }
                    }
                    Err(_) => break,
                }
            }

            if !found && let Some(tx) = tx.take() {
                let _ = tx.send(Err(anyhow!("WS URL not found in stderr")));
            }
        });

        rx.await.map_err(|_| anyhow!("Stderr reader dropped"))?
    }

    pub async fn new_tab(&self) -> Result<Tab> {
        Tab::new(self.transport.clone()).await
    }

    pub async fn capture_html(&self, html: &str, selector: &str) -> Result<String> {
        self.capture_html_with_options(html, selector, CaptureOptions::default())
            .await
    }

    pub async fn capture_html_with_options(
        &self,
        html: &str,
        selector: &str,
        opts: CaptureOptions,
    ) -> Result<String> {
        let tab = self.new_tab().await?;

        if let Some(ref viewport) = opts.viewport {
            tab.set_viewport(viewport).await?;
        }

        tab.set_content(html).await?;
        let el = tab.find_element(selector).await?;
        let shot = el.screenshot_with_options(opts).await?;
        let _ = tab.close().await;
        Ok(shot)
    }

    pub async fn capture_html_hidpi(
        &self,
        html: &str,
        selector: &str,
        scale: f64,
    ) -> Result<String> {
        let opts = CaptureOptions::new()
            .with_viewport(Viewport::default().with_device_scale_factor(scale));
        self.capture_html_with_options(html, selector, opts).await
    }

    pub async fn shutdown_global() {
        let mut lock = GLOBAL_BROWSER.lock().await;
        if let Some(browser) = lock.take() {
            let _ = browser.close_async().await;
        }
    }

    pub async fn close_async(&self) -> Result<()> {
        self.transport.shutdown().await;
        let mut lock = self.process.lock().await;
        if let Some(_proc) = lock.take() {
            // Drop triggers cleanup
        }
        Ok(())
    }

    async fn is_alive(&self) -> bool {
        self.transport
            .send(json!({
                "id": next_id(),
                "method": "Target.getTargets",
                "params": {}
            }))
            .await
            .is_ok()
    }

    /// Returns a shared singleton browser instance, launching if necessary.
    pub async fn instance() -> Self {
        Self::instance_internal(LaunchOptions::new()).await
    }

    /// Returns a shared singleton browser instance, launching with the specified path if necessary.
    ///
    /// Note: If the global browser instance is already running, this path argument will be ignored
    /// and the existing instance will be returned.
    pub async fn instance_with_path(path: impl AsRef<Path>) -> Self {
        Self::instance_internal(LaunchOptions::new().path(path)).await
    }

    /// Returns a shared singleton browser instance, launching with the given
    /// options if necessary.
    ///
    /// Note: If the global browser instance is already running, these options
    /// are ignored and the existing instance is returned.
    pub async fn instance_with_options(options: LaunchOptions) -> Self {
        Self::instance_internal(options).await
    }

    async fn instance_internal(options: LaunchOptions) -> Self {
        let mut lock = GLOBAL_BROWSER.lock().await;

        if let Some(b) = &*lock {
            if b.is_alive().await {
                return b.clone();
            }
            println!("[cdp-html-shot] Browser instance died, recreating...");
            let _ = b.close_async().await;
        }

        let b = Self::launch_with(options)
            .await
            .expect("Init global browser failed");

        // Close default blank page to save resources
        if let Ok(TransportResponse::Response(res)) = b
            .transport
            .send(json!({"id": next_id(), "method":"Target.getTargets", "params":{}}))
            .await
            && let Some(list) = res.result["targetInfos"].as_array()
            && let Some(id) = list
                .iter()
                .find(|t| t["type"] == "page")
                .and_then(|t| t["targetId"].as_str())
        {
            let _ = b
                .transport
                .send(json!({"id":next_id(), "method":"Target.closeTarget", "params":{"targetId":id}}))
                .await;
        }

        *lock = Some(b.clone());
        b
    }
}
