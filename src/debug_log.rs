use crate::APP_NAME;

#[cfg(debug_assertions)]
use std::fs::{self, File, OpenOptions};
#[cfg(debug_assertions)]
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(debug_assertions)]
use std::sync::{Mutex, Once, OnceLock};
#[cfg(debug_assertions)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(debug_assertions)]
static LOGGER: OnceLock<Option<DebugLogger>> = OnceLock::new();
#[cfg(debug_assertions)]
static INIT_LOGGED: Once = Once::new();

#[cfg(debug_assertions)]
struct DebugLogger {
    file: Mutex<File>,
    path: PathBuf,
}

pub(crate) fn init_debug_log() {
    #[cfg(debug_assertions)]
    {
        let logger = LOGGER.get_or_init(create_logger);
        INIT_LOGGED.call_once(|| {
            if let Some(logger) = logger.as_ref() {
                logger.write_line(
                    "DEBUG",
                    &format!("debug log initialized at {}", logger.path.display()),
                );
            }
        });
    }
}

#[doc(hidden)]
#[allow(dead_code)]
pub fn write_debug_log(level: &str, message: &str) {
    #[cfg(debug_assertions)]
    {
        if let Some(logger) = LOGGER.get_or_init(create_logger).as_ref() {
            logger.write_line(level, message);
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = level;
        let _ = message;
    }
}

#[cfg(debug_assertions)]
fn create_logger() -> Option<DebugLogger> {
    let directory = default_log_directory(
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
        std::env::temp_dir(),
    );

    if let Err(error) = fs::create_dir_all(&directory) {
        eprintln!(
            "MDLuma debug: failed to create debug log directory {}: {error}",
            directory.display()
        );
        return None;
    }

    let path = debug_log_file_path(&directory, std::process::id(), timestamp_millis());
    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!(
                "MDLuma debug: failed to open debug log file {}: {error}",
                path.display()
            );
            return None;
        }
    };

    Some(DebugLogger {
        file: Mutex::new(file),
        path,
    })
}

#[cfg(debug_assertions)]
impl DebugLogger {
    fn write_line(&self, level: &str, message: &str) {
        let mut file = match self.file.lock() {
            Ok(file) => file,
            Err(error) => {
                eprintln!("MDLuma debug: failed to lock debug log file: {error}");
                return;
            }
        };

        if let Err(error) = writeln!(
            file,
            "{}",
            format_log_entry(level, message, timestamp_millis())
        ) {
            eprintln!(
                "MDLuma debug: failed to write debug log file {}: {error}",
                self.path.display()
            );
            return;
        }

        if let Err(error) = file.flush() {
            eprintln!(
                "MDLuma debug: failed to flush debug log file {}: {error}",
                self.path.display()
            );
        }
    }
}

#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn default_log_directory(local_app_data: Option<PathBuf>, temp_dir: PathBuf) -> PathBuf {
    let root = local_app_data.unwrap_or(temp_dir);
    root.join(APP_NAME).join("logs")
}

#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn debug_log_file_path(base_directory: &Path, pid: u32, started_at_millis: u128) -> PathBuf {
    base_directory.join(format!("mdluma-debug-{pid}-{started_at_millis}.log"))
}

#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn format_log_entry(level: &str, message: &str, timestamp_millis: u128) -> String {
    format!("[{timestamp_millis}] [{level}] {message}")
}

#[cfg(debug_assertions)]
fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::{debug_log_file_path, default_log_directory, format_log_entry};
    use crate::APP_NAME;
    use std::path::PathBuf;

    #[test]
    fn log_directory_prefers_local_app_data() {
        let directory = default_log_directory(
            Some(PathBuf::from(r"C:\Users\ashim\AppData\Local")),
            PathBuf::from(r"C:\Temp"),
        );

        assert_eq!(
            directory,
            PathBuf::from(format!(r"C:\Users\ashim\AppData\Local\{APP_NAME}\logs"))
        );
    }

    #[test]
    fn log_directory_falls_back_to_temp_directory() {
        let directory = default_log_directory(None, PathBuf::from(r"C:\Temp"));

        assert_eq!(
            directory,
            PathBuf::from(format!(r"C:\Temp\{APP_NAME}\logs"))
        );
    }

    #[test]
    fn debug_log_file_path_uses_pid_and_timestamp() {
        let path = debug_log_file_path(PathBuf::from(r"C:\Logs").as_path(), 4242, 123456789);

        assert_eq!(
            path,
            PathBuf::from(r"C:\Logs\mdluma-debug-4242-123456789.log")
        );
    }

    #[test]
    fn formatted_log_entry_contains_timestamp_level_and_message() {
        assert_eq!(
            format_log_entry("DEBUG", "viewer diagnostic", 123456789),
            "[123456789] [DEBUG] viewer diagnostic"
        );
    }
}
