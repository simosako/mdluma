use std::path::Path;

use crate::errors::ViewerError;

pub trait ViewerChildLauncher {
    fn launch_path(&self, path: &Path, cascade_left: i32, cascade_top: i32) -> Result<(), ViewerError>;
}

impl ViewerChildLauncher for () {
    fn launch_path(&self, _path: &Path, _cascade_left: i32, _cascade_top: i32) -> Result<(), ViewerError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct ProcessViewerChildLauncher;

impl ViewerChildLauncher for ProcessViewerChildLauncher {
    fn launch_path(&self, path: &Path, cascade_left: i32, cascade_top: i32) -> Result<(), ViewerError> {
        let current_exe = std::env::current_exe().map_err(|error| {
            ViewerError::runtime_unavailable(format!(
                "cannot determine executable path for child launch: {error}"
            ))
        })?;

        std::process::Command::new(&current_exe)
            .arg(path)
            .env("MDLUMA_WINDOW_CASCADE_LEFT", cascade_left.to_string())
            .env("MDLUMA_WINDOW_CASCADE_TOP", cascade_top.to_string())
            .spawn()
            .map_err(|error| {
                ViewerError::runtime_unavailable(format!(
                    "failed to launch child instance for {}: {error}",
                    path.display()
                ))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    struct RecordingLauncher {
        launched: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl RecordingLauncher {
        fn new() -> (Self, Arc<Mutex<Vec<PathBuf>>>) {
            let launched = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                },
                launched,
            )
        }
    }

    impl ViewerChildLauncher for RecordingLauncher {
        fn launch_path(&self, path: &Path, _cascade_left: i32, _cascade_top: i32) -> Result<(), ViewerError> {
            self.launched.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }

    struct FailingLauncher {
        fail_on: PathBuf,
        error_message: String,
    }

    impl ViewerChildLauncher for FailingLauncher {
        fn launch_path(&self, path: &Path, _cascade_left: i32, _cascade_top: i32) -> Result<(), ViewerError> {
            if path == self.fail_on {
                Err(ViewerError::runtime_unavailable(&self.error_message))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn trait_records_single_path_launch() {
        let (launcher, launched) = RecordingLauncher::new();
        let path = PathBuf::from("test.md");

        launcher.launch_path(&path, 0, 0).unwrap();

        let records = launched.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0], path);
    }

    #[test]
    fn trait_launches_multiple_paths_independently() {
        let (launcher, launched) = RecordingLauncher::new();
        let paths = vec![
            PathBuf::from("a.md"),
            PathBuf::from("b.md"),
            PathBuf::from("c.md"),
        ];

        for path in &paths {
            launcher.launch_path(path, 0, 0).unwrap();
        }

        let records = launched.lock().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0], PathBuf::from("a.md"));
        assert_eq!(records[1], PathBuf::from("b.md"));
        assert_eq!(records[2], PathBuf::from("c.md"));
    }

    #[test]
    fn trait_returns_error_on_launch_failure() {
        let launcher = FailingLauncher {
            fail_on: PathBuf::from("bad.md"),
            error_message: "launch failed".to_string(),
        };

        let result = launcher.launch_path(&PathBuf::from("bad.md"), 0, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MDLuma"));
    }

    #[test]
    fn trait_success_for_non_failing_path() {
        let launcher = FailingLauncher {
            fail_on: PathBuf::from("bad.md"),
            error_message: "launch failed".to_string(),
        };

        let result = launcher.launch_path(&PathBuf::from("good.md"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn trait_one_path_one_launch_contract() {
        let (launcher, launched) = RecordingLauncher::new();
        let path = PathBuf::from("single.md");

        launcher.launch_path(&path, 0, 0).unwrap();

        let records = launched.lock().unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn process_launcher_constructs_without_arguments() {
        let _launcher = ProcessViewerChildLauncher::default();
    }
}
