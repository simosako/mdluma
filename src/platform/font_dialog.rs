use crate::errors::ViewerError;
use crate::sciter::ffi::SciterWindowHandle;
use crate::settings::BodyFontSettings;

pub trait FontDialog {
    fn choose_body_font(
        &self,
        owner: Option<SciterWindowHandle>,
        initial: Option<&BodyFontSettings>,
    ) -> Result<FontDialogResult, ViewerError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontDialogResult {
    Selected(BodyFontSettings),
    Cancelled,
}

impl FontDialog for () {
    fn choose_body_font(
        &self,
        _owner: Option<SciterWindowHandle>,
        _initial: Option<&BodyFontSettings>,
    ) -> Result<FontDialogResult, ViewerError> {
        Ok(FontDialogResult::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubFontDialog {
        result: Result<FontDialogResult, ViewerError>,
    }

    impl StubFontDialog {
        fn selected(family: &str, size_tenths: u16) -> Self {
            Self {
                result: Ok(FontDialogResult::Selected(BodyFontSettings {
                    family_name: family.to_string(),
                    point_size_tenths: size_tenths,
                })),
            }
        }

        fn cancelled() -> Self {
            Self {
                result: Ok(FontDialogResult::Cancelled),
            }
        }

        fn failed(message: &str) -> Self {
            Self {
                result: Err(ViewerError::font_dialog(message)),
            }
        }
    }

    impl FontDialog for StubFontDialog {
        fn choose_body_font(
            &self,
            _owner: Option<SciterWindowHandle>,
            _initial: Option<&BodyFontSettings>,
        ) -> Result<FontDialogResult, ViewerError> {
            self.result.clone()
        }
    }

    #[test]
    fn font_dialog_returns_selected_result() {
        let dialog = StubFontDialog::selected("Consolas", 110);
        let initial = BodyFontSettings {
            family_name: "Arial".to_string(),
            point_size_tenths: 100,
        };
        let result = dialog
            .choose_body_font(None, Some(&initial))
            .expect("dialog should succeed");
        assert!(
            matches!(result, FontDialogResult::Selected(ref bfs) if bfs.family_name == "Consolas" && bfs.point_size_tenths == 110)
        );
    }

    #[test]
    fn font_dialog_returns_cancelled_result() {
        let dialog = StubFontDialog::cancelled();
        let result = dialog
            .choose_body_font(None, None)
            .expect("cancel is not an error");
        assert!(matches!(result, FontDialogResult::Cancelled));
    }

    #[test]
    fn font_dialog_returns_error_on_failure() {
        let dialog = StubFontDialog::failed("CommDlgExtendedError: 0x0001");
        let error = dialog
            .choose_body_font(None, None)
            .expect_err("should return error");
        assert_eq!(
            error,
            ViewerError::font_dialog("CommDlgExtendedError: 0x0001")
        );
    }

    #[test]
    fn font_dialog_works_with_no_initial() {
        let dialog = StubFontDialog::selected("Segoe UI", 120);
        let result = dialog
            .choose_body_font(None, None)
            .expect("dialog should succeed");
        assert!(matches!(result, FontDialogResult::Selected(_)));
    }

    #[test]
    fn viewer_error_font_dialog_has_user_message() {
        let error = ViewerError::font_dialog("native failure");
        let msg = error.user_message();
        assert!(
            msg.contains("font"),
            "user_message should mention font: {msg}"
        );
    }

    #[test]
    fn viewer_error_font_dialog_has_operator_diagnostic() {
        let error = ViewerError::font_dialog("CommDlgExtendedError: 0x0001");
        let diag = error.operator_diagnostic();
        assert!(
            diag.contains("CommDlgExtendedError"),
            "diagnostic should contain detail: {diag}"
        );
    }
}
