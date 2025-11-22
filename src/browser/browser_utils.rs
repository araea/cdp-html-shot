use anyhow::{Context, Result};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::process::{ChildStderr, Command, Stdio};

use crate::browser::browser_config::BrowserConfig;

pub(crate) fn spawn_chrome_process(config: &BrowserConfig) -> Result<std::process::Child> {
    let mut command = Command::new(&config.executable_path);

    #[cfg(windows)]
    configure_windows_process(&mut command);

    command
        .args(config.get_browser_args())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn a Chrome process")
}

#[cfg(windows)]
fn configure_windows_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

pub(crate) async fn get_websocket_url(stderr: ChildStderr) -> Result<String> {
    let reader = BufReader::new(stderr);
    ws_url_from_reader(reader)
        .await?
        .context("Failed to get ws url")
}

async fn ws_url_from_reader(reader: BufReader<ChildStderr>) -> Result<Option<String>> {
    let re = Regex::new(r"listening on (.*/devtools/browser/.*)$")?;

    let extract = |text: &str| -> Option<String> {
        let caps = re.captures(text);
        let cap = &caps?[1];
        Some(cap.into())
    };

    for line in reader.lines() {
        if let Some(answer) = extract(&line?) {
            return Ok(Some(answer));
        }
    }
    Ok(None)
}
