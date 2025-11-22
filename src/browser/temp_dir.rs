use anyhow::{Context, Result};
use chrono::Local;
use rand::{Rng, thread_rng};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct CustomTempDir {
    path: PathBuf,
    is_cleaned: bool,
}

impl Drop for CustomTempDir {
    fn drop(&mut self) {
        if !self.is_cleaned {
            let _ = self.cleanup();
        }
    }
}

impl CustomTempDir {
    pub(crate) fn new(base_path: impl AsRef<Path>, prefix: &str) -> Result<Self> {
        let base_path = base_path.as_ref();

        fs::create_dir_all(base_path).context("Failed to create base directory")?;

        let unique_name = generate_unique_name(prefix);
        let full_path = base_path.join(unique_name);

        fs::create_dir(&full_path).context("Failed to create temporary directory")?;

        Ok(Self {
            path: full_path,
            is_cleaned: false,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn cleanup(&mut self) -> Result<()> {
        if self.is_cleaned {
            return Ok(());
        }

        fs::remove_dir_all(&self.path).context("Failed to clean up temporary directory")?;

        self.is_cleaned = true;
        Ok(())
    }
}

fn generate_unique_name(prefix: &str) -> String {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let random: String = thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    format!("{prefix}_{timestamp}_{random}")
}
