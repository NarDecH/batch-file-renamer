//! Detailed file logging for analysis and debugging.
//!
//! - Writes human-readable lines to `<data>/batch-renamer/logs/renamer-YYYY-MM-DD.log`
//!   (one file per day; the user can prune the folder at any time)
//! - Mirrors to stdout/stderr (useful for the CLI and `tauri dev`)
//! - Level controlled by the `RENAMER_LOG` env var: `error` < `warn` < `info` (default) < `debug`
//! - Thread-safe: appends are serialized behind a mutex
//!
//! Used by both the GUI and the CLI so log analysis works the same way.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

struct FileLogger {
    level: LevelFilter,
    /// Currently open day file
    file: Mutex<Option<(String, File)>>,
    log_dir: PathBuf,
    mirror_stdout: bool,
}

static LOGGER: OnceLock<FileLogger> = OnceLock::new();

pub fn log_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("batch-renamer")
        .join("logs")
}

/// Path of today's log file.
pub fn current_log_file() -> PathBuf {
    let today = chrono::Local::now().format("%Y-%m-%d");
    log_dir().join(format!("renamer-{}.log", today))
}

/// Initialize the global logger. Call once at program start.
/// `mirror_stdout`: echo every line to stdout/stderr (CLI: true, GUI: dev only).
pub fn init(mirror_stdout: bool) {
    let level = std::env::var("RENAMER_LOG")
        .ok()
        .and_then(|s| match s.to_ascii_lowercase().as_str() {
            "error" => Some(LevelFilter::Error),
            "warn" => Some(LevelFilter::Warn),
            "info" => Some(LevelFilter::Info),
            "debug" | "trace" => Some(LevelFilter::Debug),
            _ => None,
        })
        .unwrap_or(LevelFilter::Info);

    let dir = log_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("logger: cannot create log dir {:?}: {}", dir, e);
    }

    let logger = FileLogger {
        level,
        file: Mutex::new(None),
        log_dir: dir,
        mirror_stdout,
    };
    if LOGGER.set(logger).is_ok() {
        let _ = log::set_logger(LOGGER.get().unwrap());
        log::set_max_level(level);
    }
}

impl FileLogger {
    fn write_line(&self, line: &str, level: Level) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut guard = self.file.lock().unwrap();
        let rotate = match &*guard {
            Some((day, _)) => *day != today,
            None => true,
        };
        if rotate {
            let path = self.log_dir.join(format!("renamer-{}.log", today));
            match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(f) => *guard = Some((today, f)),
                Err(e) => {
                    eprintln!("logger: cannot open {:?}: {}", path, e);
                    return;
                }
            }
        }
        if let Some((_, f)) = guard.as_mut() {
            let _ = writeln!(f, "{}", line);
        }
        drop(guard);
        if self.mirror_stdout || level >= Level::Warn {
            if level <= Level::Error {
                eprintln!("{}", line);
            } else {
                println!("{}", line);
            }
        }
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let line = format!(
            "{} [{}] [{}] {}",
            ts,
            record.level(),
            record.target(),
            record.args()
        );
        self.write_line(&line, record.level());
    }

    fn flush(&self) {}
}
