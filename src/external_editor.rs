use std::path::Path;

use crate::errors::ViewerError;

pub trait ExternalEditorLauncher {
    fn launch(&self, executable: &Path, document_path: &Path) -> Result<(), ViewerError>;
}

#[derive(Default)]
pub struct ProcessExternalEditorLauncher;

impl ExternalEditorLauncher for () {
    fn launch(&self, _executable: &Path, _document_path: &Path) -> Result<(), ViewerError> {
        Ok(())
    }
}

impl ExternalEditorLauncher for ProcessExternalEditorLauncher {
    fn launch(&self, executable: &Path, document_path: &Path) -> Result<(), ViewerError> {
        std::process::Command::new(executable)
            .arg(document_path)
            .spawn()
            .map_err(|e| {
                ViewerError::external_editor_launch(executable, document_path, e.to_string())
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::{ExternalEditorLauncher, ProcessExternalEditorLauncher};
    use crate::ViewerError;

    struct FailingLauncher;

    impl ExternalEditorLauncher for FailingLauncher {
        fn launch(&self, executable: &Path, document_path: &Path) -> Result<(), ViewerError> {
            Err(ViewerError::external_editor_launch(
                executable,
                document_path,
                "injected failure for testing",
            ))
        }
    }

    struct RecordingLauncher {
        launched: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
    }

    impl RecordingLauncher {
        fn new() -> (Self, Arc<Mutex<Vec<(PathBuf, PathBuf)>>>) {
            let launched = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                },
                launched,
            )
        }
    }

    impl ExternalEditorLauncher for RecordingLauncher {
        fn launch(&self, executable: &Path, document_path: &Path) -> Result<(), ViewerError> {
            self.launched
                .lock()
                .unwrap()
                .push((executable.to_path_buf(), document_path.to_path_buf()));
            Ok(())
        }
    }

    #[test]
    fn trait_allows_mock_that_records_executable_and_document_path() {
        let (launcher, records) = RecordingLauncher::new();
        let executable = Path::new("C:\\tools\\editor.exe");
        let document_path = Path::new("C:\\docs\\notes.md");

        launcher.launch(executable, document_path).unwrap();

        let records = records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, executable);
        assert_eq!(records[0].1, document_path);
    }

    #[test]
    fn mock_launch_single_call_per_request() {
        let (launcher, records) = RecordingLauncher::new();

        launcher
            .launch(Path::new("editor.exe"), Path::new("file.md"))
            .unwrap();

        let records = records.lock().unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn process_launcher_spawn_failure_returns_external_editor_launch_error() {
        let launcher = ProcessExternalEditorLauncher::default();
        let executable = Path::new("C:\\nonexistent\\bad_editor_that_does_not_exist.exe");
        let document_path = Path::new("C:\\docs\\test.md");

        let result = launcher.launch(executable, document_path);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let user_msg = error.user_message();
        assert!(user_msg.contains("external editor"));

        let diag = error.operator_diagnostic();
        assert!(diag.contains(&executable.display().to_string()));
        assert!(diag.contains(&document_path.display().to_string()));
    }

    #[test]
    fn viewer_error_external_editor_launch_includes_paths_in_diagnostic() {
        let executable = Path::new("C:\\tools\\code.exe");
        let document_path = Path::new("C:\\project\\readme.md");

        let error = ViewerError::external_editor_launch(executable, document_path, "not found");

        let user_msg = error.user_message();
        assert!(user_msg.contains("external editor"));

        let diag = error.operator_diagnostic();
        assert!(diag.contains("code.exe"));
        assert!(diag.contains("readme.md"));
        assert!(diag.contains("not found"));
    }

    #[test]
    fn failing_launcher_returns_error_for_any_input() {
        let launcher = FailingLauncher;
        let executable = Path::new("editor.exe");
        let document_path = Path::new("doc.md");

        let result = launcher.launch(executable, document_path);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ViewerError::ExternalEditorLaunch { .. }));
    }

    #[test]
    fn failing_launcher_error_contains_executable_path_in_diagnostic() {
        let launcher = FailingLauncher;
        let executable = Path::new("C:\\tools\\vscode.exe");
        let document_path = Path::new("C:\\docs\\readme.md");

        let result = launcher.launch(executable, document_path);

        let error = result.unwrap_err();
        let diag = error.operator_diagnostic();
        assert!(
            diag.contains("vscode.exe"),
            "diagnostic must include executable path: {diag}"
        );
        assert!(
            diag.contains("readme.md"),
            "diagnostic must include document path: {diag}"
        );
    }
}
