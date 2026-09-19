use crate::tab::Tab;
use crate::transport::{Transport, TransportResponse, next_id};
use crate::types::{CaptureOptions, Viewport};
use anyhow::{Context, Result, anyhow};
use rand::{RngExt, rng};
use regex::Regex;
use serde_json::json;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use which::which;

/// The name every throwaway profile directory starts with.
const PROFILE_PREFIX: &str = "cdp-shot";

/// File whose lock marks a profile as owned, held for the profile's whole life.
const OWNER_LOCK: &str = ".cdp-html-shot.lock";

/// How young a profile must be to be spared a sweep.
///
/// A profile is created a moment before its lock is taken, and a sweep landing
/// in that gap would mistake a brand-new profile for an orphan. The age floor
/// costs nothing, because the lock — not the age — is what tells a dead owner's
/// profile from a live one.
#[cfg(unix)]
const SWEEP_GRACE: Duration = Duration::from_secs(60);

/// Chromium feature that downloads the 2.8 GB on-device optimization model.
///
/// Every fresh profile would fetch it again, which turns a leftover profile
/// from a rounding error into a real cost.
const MODEL_DOWNLOAD_FEATURE: &str = "OptimizationGuideModelDownloading";

/// Temporary directory for browser user data, deleted on drop.
///
/// The directory carries a locked file for as long as its owner lives. The
/// kernel releases that lock when the owning process dies, however it dies,
/// which is what lets [`sweep_orphaned_profiles`] separate a profile whose
/// owner is gone from one that is still in use.
#[derive(Debug)]
struct CustomTempDir {
    path: PathBuf,
    /// Released in `drop` before the directory goes away: on Windows an open
    /// handle inside a directory blocks its removal.
    lock: Option<File>,
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
        match Self::claim(&path) {
            Ok(lock) => Ok(Self {
                path,
                lock: Some(lock),
            }),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&path);
                Err(e.into())
            }
        }
    }

    /// Take the ownership lock on a profile directory.
    ///
    /// Fails while another live process holds it.
    fn claim(path: &Path) -> std::io::Result<File> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.join(OWNER_LOCK))?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: `file` owns the descriptor for the duration of the call.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(file)
    }

    /// Whether the directory's owner is gone.
    ///
    /// A profile only counts when it carries our lock file. Directories without
    /// one were not created by a version that takes the lock, and such a profile
    /// may be very much in use — an older build of this library, or a different
    /// program that happens to use the same prefix. Being unable to tell a live
    /// profile from a dead one, the sweep has to leave it alone.
    ///
    /// Only Unix can answer: elsewhere [`Self::claim`] takes no real lock, so a
    /// profile must never be swept on the strength of it.
    #[cfg(unix)]
    fn is_orphaned(path: &Path) -> bool {
        if !path.join(OWNER_LOCK).is_file() {
            return false;
        }
        Self::claim(path).is_ok()
    }
}

impl Drop for CustomTempDir {
    fn drop(&mut self) {
        // Let go of the lock first: an open handle inside the directory keeps
        // Windows from removing it.
        self.lock = None;
        for i in 0..10 {
            if std::fs::remove_dir_all(&self.path).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100 * (i as u64 + 1).min(3)));
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Delete profiles whose owning process died without running its destructor.
///
/// [`CustomTempDir::drop`] is the orderly path, and a host that is SIGKILLed,
/// aborts on a panic or leaves through `std::process::exit()` never reaches it.
/// The lock it held, though, the kernel releases no matter how it died. Sweeping
/// at launch turns that into self-healing, which is what the profiles need where
/// nothing else cleans the temp directory: Termux's `$PREFIX/tmp` is never
/// swept by the system, so a leftover profile there is leftover forever.
///
/// Unix only; see [`CustomTempDir::is_orphaned`].
#[cfg(unix)]
fn sweep_orphaned_profiles(root: &Path, grace: Duration) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(rest) = name
            .to_str()
            .and_then(|name| name.strip_prefix(PROFILE_PREFIX))
        else {
            continue;
        };
        if !rest.starts_with('_') {
            continue;
        }
        let path = entry.path();
        // `symlink_metadata` on purpose: a symlink points somewhere that is not
        // ours to delete.
        if !path.symlink_metadata().is_ok_and(|m| m.is_dir()) {
            continue;
        }
        let young = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < grace);
        if young {
            continue;
        }
        if CustomTempDir::is_orphaned(&path) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

#[derive(Debug)]
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

/// Ask the kernel to kill the browser when its parent goes away.
///
/// [`BrowserProcess::drop`] only runs on an orderly teardown. A host that is
/// SIGKILLed, aborts on a panic, or leaves through `std::process::exit()` never
/// gets there, and the browser survives as an orphan that holds memory, keeps
/// its `--user-data-dir` open and stays connected to nothing. `PR_SET_PDEATHSIG`
/// makes the kernel send SIGKILL in that case, so the browser cannot outlive
/// whoever launched it.
///
/// The kernel watches the parent **thread**, not the parent process. The spawn
/// happens on the caller's thread, so keep `launch_with` off short-lived threads:
/// a `spawn_blocking` thread is reaped after an idle timeout, which would take
/// the browser down with it. The one `spawn_blocking` in this file only drains
/// stderr, after the browser is already up, so it is not affected.
///
/// The parent can die between `fork` and `prctl`, in which case the signal will
/// never be delivered — hence the `getppid` check, which lets the child notice
/// and quit on its own.
#[cfg(unix)]
fn arm_parent_death_signal(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    let parent = unsafe { libc::getpid() };
    // SAFETY: `pre_exec` runs between fork and exec, where only async-signal-safe
    // calls are sound. prctl, getppid and _exit all are; nothing allocates.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                libc::_exit(1);
            }
            Ok(())
        });
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

#[derive(Clone, Debug)]
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
        // A host that died without cleaning up left its profile behind; take the
        // chance to clear it out before adding another one.
        #[cfg(unix)]
        sweep_orphaned_profiles(&root, SWEEP_GRACE);
        let temp = CustomTempDir::new(root, PROFILE_PREFIX)?;
        let exe = Self::find_chrome(options.path.clone())?;
        let port = (8000..9000)
            .find(|&p| std::net::TcpListener::bind(("127.0.0.1", p)).is_ok())
            .ok_or(anyhow!("No available port"))?;

        let user_agent = options
            .user_agent
            .clone()
            .or_else(|| Self::default_user_agent(&exe));
        let args = Self::build_args(port, &temp.path, &options, user_agent);

        #[cfg(windows)]
        let mut cmd = {
            use std::os::windows::process::CommandExt;
            let mut c = Command::new(&exe);
            c.creation_flags(0x08000000);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = Command::new(&exe);

        // Keep the browser from outliving us even when nothing gets to run on the way out.
        #[cfg(unix)]
        arm_parent_death_signal(&mut cmd);

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

    /// The full Chromium command line for one launch.
    ///
    /// Split out from [`Browser::launch_with`] so the ordering rule this
    /// crate promises — caller switches last, overriding the built-ins — is
    /// checkable without starting a browser.
    fn build_args(
        port: u16,
        user_data_dir: &Path,
        options: &LaunchOptions,
        user_agent: Option<String>,
    ) -> Vec<String> {
        let mut args = vec![
            format!("--remote-debugging-port={port}"),
            format!("--user-data-dir={}", user_data_dir.display()),
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
            // Fresh profiles are throwaway, so the on-device model is dead weight
            // at 2.8 GB a fetch. A caller's own `--disable-features` reintroduces
            // this entry rather than replacing it — see below.
            format!("--disable-features={MODEL_DOWNLOAD_FEATURE}"),
        ];
        args.extend(GPU_ARGS.iter().map(|a| a.to_string()));

        if let Some(agent) = user_agent {
            args.push(format!("--user-agent={agent}"));
        }
        if options.headless {
            args.push("--headless=new".into());
        }

        // Last, so a caller can override any of the above: Chromium honours
        // the last occurrence of a repeated switch.
        for arg in &options.extra_args {
            // Chromium keeps only the last `--disable-features`, so a caller
            // naming one feature would otherwise switch the model download back
            // on. Fold our entry into the caller's list instead.
            match arg.strip_prefix("--disable-features=") {
                Some(extra) => args.push(format!(
                    "--disable-features={MODEL_DOWNLOAD_FEATURE},{extra}"
                )),
                None => args.push(arg.clone()),
            }
        }
        args
    }

    /// The major version out of a `--version` line.
    ///
    /// `"Chromium 149.0.7827.155"`, `"Google Chrome 131.0.6778.86"` and
    /// `"Microsoft Edge 130.0.2849.68"` all reduce to their leading number.
    /// A version is `major.minor.build.patch`; anything with a different shape
    /// is a product name or a suffix, not a version.
    fn parse_major_version(version_output: &str) -> Option<u32> {
        version_output.split_whitespace().find_map(|token| {
            let mut parts = token.split('.');
            let major = parts.next()?.parse::<u32>().ok()?;
            (parts.count() == 3).then_some(major)
        })
    }

    /// An ordinary desktop user agent for the given major version.
    fn user_agent_for(major: u32) -> String {
        // Android runs the ordinary Linux build under Termux, and its client
        // hints say so, so it takes the Linux token too.
        let platform = if cfg!(target_os = "windows") {
            "Windows NT 10.0; Win64; x64"
        } else if cfg!(target_os = "macos") {
            "Macintosh; Intel Mac OS X 10_15_7"
        } else {
            "X11; Linux x86_64"
        };

        format!(
            "Mozilla/5.0 ({platform}) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/{major}.0.0.0 Safari/537.36"
        )
    }

    /// Where the throwaway user-data directory is created.
    ///
    /// The system temp directory, so that a browser killed before its `Drop`
    /// runs leaves its profile somewhere a later launch can reclaim it —
    /// [`sweep_orphaned_profiles`] does that on Unix. Earlier versions used
    /// `./temp`, which littered whatever project the caller happened to
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
        let major = Self::parse_major_version(&String::from_utf8_lossy(&output.stdout))?;
        Some(Self::user_agent_for(major))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args_for(options: &LaunchOptions) -> Vec<String> {
        Browser::build_args(
            9222,
            Path::new("/tmp/profile"),
            options,
            Some("agent/1".into()),
        )
    }

    fn position_of(args: &[String], needle: &str) -> Option<usize> {
        args.iter().position(|a| a == needle)
    }

    #[test]
    fn defaults_are_headless_and_carry_the_profile() {
        let args = args_for(&LaunchOptions::new());

        assert!(args.contains(&"--headless=new".to_string()));
        assert!(args.contains(&"--remote-debugging-port=9222".to_string()));
        assert!(args.contains(&"--user-data-dir=/tmp/profile".to_string()));
        assert!(args.contains(&"--user-agent=agent/1".to_string()));
    }

    #[test]
    fn headed_mode_omits_the_headless_switch() {
        let args = args_for(&LaunchOptions::new().headless(false));
        assert!(!args.iter().any(|a| a.starts_with("--headless")));
    }

    /// The whole point of accepting extra switches: Chromium honours the last
    /// occurrence, so a caller's switch must sit after every built-in one.
    #[test]
    fn caller_switches_come_after_every_builtin() {
        let options = LaunchOptions::new()
            .arg("--window-size=100,100")
            .args(["--lang=zh-CN", "--proxy-server=http://127.0.0.1:8080"]);
        let args = args_for(&options);

        let builtin = position_of(&args, "--no-sandbox").expect("built-in switch missing");
        for caller in [
            "--window-size=100,100",
            "--lang=zh-CN",
            "--proxy-server=http://127.0.0.1:8080",
        ] {
            let at = position_of(&args, caller).unwrap_or_else(|| panic!("{caller} missing"));
            assert!(at > builtin, "{caller} must be able to override built-ins");
        }

        // The built-in window size is still present; the caller's copy wins by
        // being later, which is exactly the contract being asserted.
        let first = position_of(&args, "--window-size=1200,1600").unwrap();
        let last = position_of(&args, "--window-size=100,100").unwrap();
        assert!(first < last);
    }

    #[test]
    fn no_user_agent_switch_when_the_version_is_unreadable() {
        let args = Browser::build_args(9222, Path::new("/tmp/p"), &LaunchOptions::new(), None);
        assert!(!args.iter().any(|a| a.starts_with("--user-agent")));
    }

    /// Android hosts the GPU service out of process; everywhere else it is
    /// folded into the browser process. Getting this backwards costs a crash
    /// that only shows up under load, so it is worth pinning down.
    #[test]
    fn gpu_placement_matches_the_platform() {
        let args = args_for(&LaunchOptions::new());
        if cfg!(target_os = "android") {
            assert!(args.contains(&"--disable-gpu".to_string()));
            assert!(args.contains(&"--enable-unsafe-swiftshader".to_string()));
            assert!(!args.contains(&"--in-process-gpu".to_string()));
        } else {
            assert!(args.contains(&"--in-process-gpu".to_string()));
            assert!(!args.contains(&"--disable-gpu".to_string()));
        }
    }

    #[test]
    fn version_lines_reduce_to_their_major() {
        for (line, want) in [
            ("Chromium 149.0.7827.155", Some(149)),
            ("Google Chrome 131.0.6778.86 ", Some(131)),
            ("Microsoft Edge 130.0.2849.68", Some(130)),
            ("Chromium 120.0.6099.109 snap", Some(120)),
        ] {
            assert_eq!(Browser::parse_major_version(line), want, "line: {line}");
        }
    }

    #[test]
    fn non_versions_are_rejected_rather_than_guessed() {
        for line in ["Chromium", "", "not.a.version", "1.2 3.4"] {
            assert_eq!(Browser::parse_major_version(line), None, "line: {line}");
        }
    }

    /// The `Headless` token is what the override exists to remove; keeping the
    /// real major version is what keeps it consistent with the client hints.
    #[test]
    fn generated_user_agent_hides_headless_and_keeps_the_version() {
        let agent = Browser::user_agent_for(149);
        assert!(!agent.contains("Headless"));
        assert!(agent.contains("Chrome/149.0.0.0"));
        assert!(agent.starts_with("Mozilla/5.0 ("));
    }

    #[test]
    fn temp_profile_is_created_and_removed_with_its_guard() {
        let root = std::env::temp_dir().join("cdp-html-shot-test-root");
        let path = {
            let temp = CustomTempDir::new(root.clone(), "unit").expect("create");
            assert!(temp.path.is_dir());
            temp.path.clone()
        };
        assert!(!path.exists(), "the guard must remove the profile on drop");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn user_data_root_is_writable() {
        let root = Browser::user_data_root();
        assert!(root.is_dir(), "expected a usable directory, got {root:?}");
    }

    /// The kernel kills the child once its parent thread exits. This is the
    /// guarantee that keeps a browser from outliving a SIGKILLed host.
    /// Reads `/proc`, so it is limited to Linux and Android.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn parent_death_signal_kills_the_child_when_the_parent_thread_ends() {
        use std::time::{Duration, Instant};

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut cmd = Command::new("sleep");
            cmd.arg("30");
            arm_parent_death_signal(&mut cmd);
            let child = cmd.spawn().expect("spawn sleep");
            tx.send(child.id()).expect("send pid");
            // The thread ends here, which is exactly what PDEATHSIG watches.
        });
        let pid = rx.recv().expect("pid from the spawning thread");

        // Nobody waits on the child, so a killed child stays a zombie; both the
        // zombie state and a vanished entry mean the signal arrived.
        let state = |pid: u32| -> Option<char> {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            let (_, rest) = stat.rsplit_once(") ")?;
            rest.chars().next()
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match state(pid) {
                None | Some('Z') | Some('X') => return,
                _ => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        // SAFETY: best-effort cleanup so a failing test does not leave `sleep` behind.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        panic!(
            "child {pid} outlived its parent thread: state {:?}",
            state(pid)
        );
    }

    fn test_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "cdp-html-shot-sweep-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&root).expect("create test root");
        root
    }

    /// A profile whose owner was killed without unwinding keeps its directory.
    /// An unlocked lock file is exactly what that leaves behind.
    #[cfg(unix)]
    #[test]
    fn sweep_removes_a_profile_whose_owner_is_gone() {
        let root = test_root("orphan");
        let path = root.join(format!("{PROFILE_PREFIX}_20260101_000000_orphan"));
        std::fs::create_dir(&path).expect("create profile");
        std::fs::write(path.join(OWNER_LOCK), b"").expect("write lock file");

        sweep_orphaned_profiles(&root, Duration::ZERO);

        assert!(!path.exists(), "an unowned profile must be swept");
        let _ = std::fs::remove_dir_all(root);
    }

    /// A profile without our lock file may belong to an older build that is
    /// still running, so it is never the sweep's to judge.
    #[cfg(unix)]
    #[test]
    fn sweep_spares_a_profile_without_a_lock_file() {
        let root = test_root("lockless");
        let path = root.join(format!("{PROFILE_PREFIX}_20260101_000000_legacy"));
        std::fs::create_dir(&path).expect("create profile");

        sweep_orphaned_profiles(&root, Duration::ZERO);

        assert!(path.is_dir(), "a lock-less profile must be left alone");
        let _ = std::fs::remove_dir_all(root);
    }

    /// The sweep must never touch a profile that is still in use.
    #[cfg(unix)]
    #[test]
    fn sweep_spares_a_profile_that_is_still_owned() {
        let root = test_root("live");
        let live = CustomTempDir::new(root.clone(), PROFILE_PREFIX).expect("create");

        sweep_orphaned_profiles(&root, Duration::ZERO);

        assert!(
            live.path.is_dir(),
            "a locked profile must survive the sweep"
        );
        drop(live);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Only our own directories are eligible: a shared temp root holds plenty
    /// of other things, and a lookalike name must not become collateral.
    #[cfg(unix)]
    #[test]
    fn sweep_leaves_unrelated_entries_alone() {
        let root = test_root("mixed");
        let other = root.join("cdp-shot-not-ours");
        std::fs::create_dir(&other).expect("create other");

        sweep_orphaned_profiles(&root, Duration::ZERO);

        assert!(other.is_dir(), "a lookalike name must not be swept");
        let _ = std::fs::remove_dir_all(root);
    }

    /// A profile is created a moment before its lock is taken. A sweep landing
    /// in that gap would delete a concurrent launch's brand-new profile.
    #[cfg(unix)]
    #[test]
    fn sweep_spares_a_freshly_created_profile() {
        let root = test_root("fresh");
        let path = root.join(format!("{PROFILE_PREFIX}_20260101_000000_abcdef"));
        std::fs::create_dir(&path).expect("create profile");

        sweep_orphaned_profiles(&root, Duration::from_secs(60));

        assert!(path.is_dir(), "a fresh profile must survive");
        let _ = std::fs::remove_dir_all(root);
    }

    /// Every fresh profile would otherwise fetch the on-device model again, so
    /// the switch has to be there by default.
    #[test]
    fn the_on_device_model_download_is_off_by_default() {
        let args = args_for(&LaunchOptions::new());
        assert!(
            args.iter()
                .any(|a| a == &format!("--disable-features={MODEL_DOWNLOAD_FEATURE}")),
            "expected the model download to be off, got {args:?}"
        );
    }

    /// Chromium keeps only the last `--disable-features`, so a caller naming one
    /// feature must not switch the model download back on.
    #[test]
    fn a_caller_disable_features_keeps_the_model_download_off() {
        let args = args_for(&LaunchOptions::new().arg("--disable-features=Translate"));

        let last = args
            .iter()
            .filter_map(|a| a.strip_prefix("--disable-features="))
            .next_back()
            .expect("a --disable-features switch");
        assert!(last.contains(MODEL_DOWNLOAD_FEATURE), "got {last}");
        assert!(last.contains("Translate"), "got {last}");
    }
}
