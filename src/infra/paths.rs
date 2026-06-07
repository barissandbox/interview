//! Application path helpers.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// File paths used by the application.
#[derive(Clone, Debug)]
pub struct AppPaths {
    /// Application data directory.
    pub data_dir: PathBuf,
    /// JSON settings path.
    pub settings: PathBuf,
    /// ChatGPT auth JSON path.
    pub auth: PathBuf,
    /// CV/profile JSON path.
    pub profile: PathBuf,
    /// Model catalog JSON path.
    pub catalog: PathBuf,
    /// Transcript JSON directory.
    pub transcripts: PathBuf,
    /// Log file path.
    pub log_file: PathBuf,
}

const APP_DATA_DIR: &str = "Interview";

/// Resolves and creates the application data directory.
pub fn app_paths() -> Result<AppPaths> {
    let data_root = dirs::data_dir().context("Could not resolve user data directory")?;
    let data_dir = data_root.join(APP_DATA_DIR);
    let transcripts = data_dir.join("data");
    std::fs::create_dir_all(&data_dir).context("Could not create app data directory")?;
    std::fs::create_dir_all(&transcripts).context("Could not create transcript data directory")?;
    Ok(AppPaths {
        settings: data_dir.join("settings.json"),
        auth: data_dir.join("auth.json"),
        profile: data_dir.join("profile.json"),
        catalog: data_dir.join("catalog.json"),
        log_file: data_dir.join("app.log"),
        transcripts,
        data_dir,
    })
}
