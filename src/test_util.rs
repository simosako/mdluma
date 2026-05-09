use crate::errors::ViewerError;
use crate::external_editor::ExternalEditorLauncher;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn unique_test_dir(name: &str) -> TestDir {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("mdluma-{name}-{nonce}"));
    TestDir { path }
}

#[derive(Debug)]
pub(crate) struct TestDir {
    path: PathBuf,
}

impl AsRef<Path> for TestDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl std::ops::Deref for TestDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(crate) struct RecordingExternalEditorLauncher {
    launched: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
}

impl RecordingExternalEditorLauncher {
    pub(crate) fn new() -> (Self, Arc<Mutex<Vec<(PathBuf, PathBuf)>>>) {
        let launched = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                launched: launched.clone(),
            },
            launched,
        )
    }
}

impl ExternalEditorLauncher for RecordingExternalEditorLauncher {
    fn launch(&self, executable: &Path, document_path: &Path) -> Result<(), ViewerError> {
        self.launched
            .lock()
            .unwrap()
            .push((executable.to_path_buf(), document_path.to_path_buf()));
        Ok(())
    }
}
