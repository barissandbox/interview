//! File-backed logger implementation.

use anyhow::Result;
use chrono::Utc;
use log::{LevelFilter, Metadata, Record};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// Minimal logger that appends records to a local log file.
struct FileLogger {
    path: PathBuf,
}

impl log::Log for FileLogger {
    /// Checks whether a log record is enabled.
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    /// Writes a log record to the log file.
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(
                file,
                "{} [{}] {} - {}",
                Utc::now().to_rfc3339(),
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    /// Flushes pending log writes.
    fn flush(&self) {}
}

/// Installs the file logger.
pub fn install_logger(path: PathBuf) -> Result<()> {
    let logger = FileLogger { path };
    let _ = log::set_boxed_logger(Box::new(logger));
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
