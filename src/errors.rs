use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupError {
    message: String,
}

impl StartupError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn from_viewer_error(error: ViewerError) -> Self {
        Self::new(format!(
            "{} Diagnostic: {}",
            error.user_message(),
            error.operator_diagnostic()
        ))
    }
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StartupError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerError {
    RuntimeMissing {
        path: PathBuf,
    },
    RuntimeUnavailable {
        message: String,
    },
    FileDialog {
        message: String,
    },
    FileRead {
        path: PathBuf,
        message: String,
    },
    InvalidEncoding {
        path: PathBuf,
    },
    MarkdownRender {
        message: String,
    },
    FontDialog {
        message: String,
    },
    Ui {
        message: String,
    },
    ExternalEditorLaunch {
        executable: PathBuf,
        document_path: PathBuf,
        message: String,
    },
    SettingsSave {
        path: PathBuf,
        message: String,
    },
}

impl ViewerError {
    pub fn runtime_missing(path: impl AsRef<Path>) -> Self {
        Self::RuntimeMissing {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn runtime_unavailable(message: impl Into<String>) -> Self {
        Self::RuntimeUnavailable {
            message: message.into(),
        }
    }

    pub fn file_dialog(message: impl Into<String>) -> Self {
        Self::FileDialog {
            message: message.into(),
        }
    }

    pub fn file_read(path: impl AsRef<Path>, message: impl Into<String>) -> Self {
        Self::FileRead {
            path: path.as_ref().to_path_buf(),
            message: message.into(),
        }
    }

    pub fn invalid_encoding(path: impl AsRef<Path>) -> Self {
        Self::InvalidEncoding {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn markdown_render(message: impl Into<String>) -> Self {
        Self::MarkdownRender {
            message: message.into(),
        }
    }

    pub fn ui(message: impl Into<String>) -> Self {
        Self::Ui {
            message: message.into(),
        }
    }

    pub fn font_dialog(message: impl Into<String>) -> Self {
        Self::FontDialog {
            message: message.into(),
        }
    }

    pub fn settings_save(path: impl AsRef<Path>, message: impl Into<String>) -> Self {
        Self::SettingsSave {
            path: path.as_ref().to_path_buf(),
            message: message.into(),
        }
    }

    pub fn external_editor_launch(
        executable: &Path,
        document_path: &Path,
        message: impl Into<String>,
    ) -> Self {
        Self::ExternalEditorLaunch {
            executable: executable.to_path_buf(),
            document_path: document_path.to_path_buf(),
            message: message.into(),
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::RuntimeMissing { .. } => {
                "MDLuma cannot start because a required runtime file is missing.".to_string()
            }
            Self::RuntimeUnavailable { .. } => {
                "MDLuma cannot start because the rendering runtime is unavailable.".to_string()
            }
            Self::FileDialog { .. } => {
                "MDLuma could not open the file selection dialog.".to_string()
            }
            Self::FileRead { .. } => {
                "MDLuma could not read the selected Markdown file.".to_string()
            }
            Self::InvalidEncoding { .. } => {
                "MDLuma can only open UTF-8 Markdown files.".to_string()
            }
            Self::MarkdownRender { .. } => {
                "MDLuma could not render the Markdown document.".to_string()
            }
            Self::Ui { .. } => "MDLuma could not update the viewer window.".to_string(),
            Self::FontDialog { .. } => {
                "MDLuma could not open the font selection dialog.".to_string()
            }
            Self::ExternalEditorLaunch { .. } => {
                "MDLuma could not open the file in the external editor.".to_string()
            }
            Self::SettingsSave { .. } => {
                "MDLuma could not save application settings.".to_string()
            }
        }
    }

    pub fn operator_diagnostic(&self) -> String {
        match self {
            Self::RuntimeMissing { path } => {
                format!("missing runtime file: expected {}", path.display())
            }
            Self::RuntimeUnavailable { message } => format!("runtime unavailable: {message}"),
            Self::FileDialog { message } => format!("file dialog failed: {message}"),
            Self::FileRead { path, message } => {
                format!("failed to read {}: {message}", path.display())
            }
            Self::InvalidEncoding { path } => {
                format!("invalid UTF-8 in Markdown file: {}", path.display())
            }
            Self::MarkdownRender { message } => format!("markdown render failed: {message}"),
            Self::Ui { message } => format!("UI update failed: {message}"),
            Self::FontDialog { message } => format!("font dialog failed: {message}"),
            Self::ExternalEditorLaunch {
                executable,
                document_path,
                message,
            } => {
                format!(
                    "external editor launch failed: executable={}, document={}, error={message}",
                    executable.display(),
                    document_path.display()
                )
            }
            Self::SettingsSave { path, message } => {
                format!(
                    "settings save failed: {} - {message}",
                    path.display()
                )
            }
        }
    }
}

impl fmt::Display for ViewerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.user_message())
    }
}

impl std::error::Error for ViewerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn external_editor_launch_is_distinct_variant() {
        let err = ViewerError::external_editor_launch(
            Path::new("C:\\editor.exe"),
            Path::new("C:\\doc.md"),
            "os error",
        );
        assert!(matches!(err, ViewerError::ExternalEditorLaunch { .. }));
        assert!(!matches!(err, ViewerError::Ui { .. }));
        assert!(!matches!(err, ViewerError::FileDialog { .. }));
    }

    #[test]
    fn external_editor_launch_user_message_mentions_external_editor() {
        let err = ViewerError::external_editor_launch(
            Path::new("editor.exe"),
            Path::new("doc.md"),
            "fail",
        );
        let msg = err.user_message();
        assert!(
            msg.to_lowercase().contains("external editor"),
            "user message should mention 'external editor': {msg}"
        );
        assert!(
            !msg.contains("editor.exe"),
            "user message must not expose internal paths: {msg}"
        );
        assert!(
            !msg.contains("fail"),
            "user message must not expose raw error details: {msg}"
        );
    }

    #[test]
    fn external_editor_launch_diagnostic_includes_executable_and_document_path() {
        let exe = Path::new("C:\\tools\\code.exe");
        let doc = Path::new("C:\\project\\readme.md");
        let err = ViewerError::external_editor_launch(exe, doc, "access denied");

        let diag = err.operator_diagnostic();
        assert!(
            diag.contains("code.exe"),
            "diagnostic must include executable path: {diag}"
        );
        assert!(
            diag.contains("readme.md"),
            "diagnostic must include document path: {diag}"
        );
        assert!(
            diag.contains("access denied"),
            "diagnostic must include error message: {diag}"
        );
    }

    #[test]
    fn configured_editor_failure_is_identifiable_from_executable_field() {
        let configured = Path::new("C:\\tools\\my_custom_editor.exe");
        let fallback = Path::new("notepad.exe");

        let err_configured =
            ViewerError::external_editor_launch(configured, Path::new("file.md"), "not found");
        let err_fallback =
            ViewerError::external_editor_launch(fallback, Path::new("file.md"), "spawn failed");

        if let ViewerError::ExternalEditorLaunch { executable, .. } = &err_configured {
            assert_eq!(executable, configured);
        } else {
            panic!("expected ExternalEditorLaunch variant");
        }
        if let ViewerError::ExternalEditorLaunch { executable, .. } = &err_fallback {
            assert_eq!(executable, fallback);
        } else {
            panic!("expected ExternalEditorLaunch variant");
        }
    }

    #[test]
    fn display_delegates_to_user_message() {
        let err = ViewerError::external_editor_launch(Path::new("e.exe"), Path::new("f.md"), "err");
        assert_eq!(err.to_string(), err.user_message());
    }
}
