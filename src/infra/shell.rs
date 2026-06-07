//! Platform shell helpers.

use anyhow::{Context, Result};
use std::process::Command;

/// Opens an external URL with the platform default handler.
pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("rundll32");
        command.args(["url.dll,FileProtocolHandler", url]);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    command
        .spawn()
        .context("Could not open URL in the default browser")?;
    Ok(())
}
