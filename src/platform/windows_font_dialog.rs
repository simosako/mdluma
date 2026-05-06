use crate::errors::ViewerError;
use crate::platform::font_dialog::{FontDialog, FontDialogResult};
use crate::sciter::ffi::SciterWindowHandle;
use crate::settings::BodyFontSettings;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsFontDialog;

impl FontDialog for WindowsFontDialog {
    fn choose_body_font(
        &self,
        owner: Option<SciterWindowHandle>,
        initial: Option<&BodyFontSettings>,
    ) -> Result<FontDialogResult, ViewerError> {
        choose_body_font_with(initial, owner, runtime_choose_font)
    }
}

enum FontPickOutcome {
    Selected {
        family_name: String,
        point_size_tenths: u16,
    },
    Cancelled,
}

fn choose_body_font_with(
    initial: Option<&BodyFontSettings>,
    owner: Option<SciterWindowHandle>,
    picker: impl FnOnce(
        Option<SciterWindowHandle>,
        Option<&BodyFontSettings>,
    ) -> Result<FontPickOutcome, ViewerError>,
) -> Result<FontDialogResult, ViewerError> {
    match picker(owner, initial)? {
        FontPickOutcome::Selected {
            family_name,
            point_size_tenths,
        } => Ok(FontDialogResult::Selected(BodyFontSettings {
            family_name,
            point_size_tenths,
        })),
        FontPickOutcome::Cancelled => Ok(FontDialogResult::Cancelled),
    }
}

#[cfg(windows)]
fn runtime_choose_font(
    owner: Option<SciterWindowHandle>,
    initial: Option<&BodyFontSettings>,
) -> Result<FontPickOutcome, ViewerError> {
    use std::ffi::OsString;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::null_mut;

    use windows_sys::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, LOGFONTW};
    use windows_sys::Win32::UI::Controls::Dialogs::{
        ChooseFontW, CommDlgExtendedError, CF_FORCEFONTEXIST, CF_INITTOLOGFONTSTRUCT, CHOOSEFONTW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, IsWindow, GA_ROOT};

    let owner = owner.and_then(|hwnd| {
        if unsafe { IsWindow(hwnd) } == 0 {
            None
        } else {
            let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
            if root.is_null() {
                Some(hwnd)
            } else {
                Some(root)
            }
        }
    });

    let mut lf: LOGFONTW = unsafe { zeroed() };
    if let Some(settings) = initial {
        let wide: Vec<u16> = settings
            .family_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let copy_len = wide.len().min(lf.lfFaceName.len());
        lf.lfFaceName[..copy_len].copy_from_slice(&wide[..copy_len]);

        let hdc = unsafe { GetDC(null_mut()) };
        let dpi = if hdc != null_mut() {
            let d = unsafe { GetDeviceCaps(hdc, 90) };
            unsafe { ReleaseDC(null_mut(), hdc) };
            d
        } else {
            96
        };
        lf.lfHeight = -((settings.point_size_tenths as i64 * dpi as i64) / 720) as i32;
    }

    let mut cf: CHOOSEFONTW = unsafe { zeroed() };
    cf.lStructSize = size_of::<CHOOSEFONTW>() as u32;
    cf.lpLogFont = &mut lf;
    cf.Flags = CF_INITTOLOGFONTSTRUCT | CF_FORCEFONTEXIST;
    cf.hwndOwner = owner.unwrap_or(null_mut());

    let result = unsafe { ChooseFontW(&mut cf) };
    if result != 0 {
        let end = lf
            .lfFaceName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(lf.lfFaceName.len());
        let family = OsString::from_wide(&lf.lfFaceName[..end])
            .to_string_lossy()
            .into_owned();
        return Ok(FontPickOutcome::Selected {
            family_name: family,
            point_size_tenths: cf.iPointSize as u16,
        });
    }

    let error = unsafe { CommDlgExtendedError() };
    if error == 0 {
        Ok(FontPickOutcome::Cancelled)
    } else {
        Err(ViewerError::font_dialog(format!(
            "Windows font dialog failed with code 0x{error:04x}"
        )))
    }
}

#[cfg(not(windows))]
fn runtime_choose_font(
    _owner: Option<SciterWindowHandle>,
    _initial: Option<&BodyFontSettings>,
) -> Result<FontPickOutcome, ViewerError> {
    Err(ViewerError::font_dialog(
        "Windows font dialog is unavailable on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::BodyFontSettings;

    fn sample_settings() -> BodyFontSettings {
        BodyFontSettings {
            family_name: "Consolas".to_string(),
            point_size_tenths: 110,
        }
    }

    #[test]
    fn windows_font_dialog_implements_font_dialog_trait() {
        let dialog = WindowsFontDialog;
        let _: &dyn FontDialog = &dialog;
    }

    #[test]
    fn selected_font_is_returned_as_selected_result() {
        let initial = sample_settings();
        let result = choose_body_font_with(Some(&initial), None, |owner, init| {
            assert!(owner.is_none());
            let s = init.unwrap();
            Ok(FontPickOutcome::Selected {
                family_name: s.family_name.clone(),
                point_size_tenths: s.point_size_tenths,
            })
        })
        .expect("choose body font");

        match result {
            FontDialogResult::Selected(bfs) => {
                assert_eq!(bfs.family_name, "Consolas");
                assert_eq!(bfs.point_size_tenths, 110);
            }
            FontDialogResult::Cancelled => panic!("should not be cancelled"),
        }
    }

    #[test]
    fn cancel_is_reported_without_treating_it_as_error() {
        let result = choose_body_font_with(None, None, |owner, _| {
            assert!(owner.is_none());
            Ok(FontPickOutcome::Cancelled)
        })
        .expect("cancel dialog");

        assert!(matches!(result, FontDialogResult::Cancelled));
    }

    #[test]
    fn picker_error_is_mapped_to_font_dialog_error() {
        let error = choose_body_font_with(None, None, |_, _| {
            Err(ViewerError::font_dialog("native dialog failed"))
        })
        .expect_err("propagate dialog error");

        assert_eq!(error, ViewerError::font_dialog("native dialog failed"));
    }

    #[test]
    fn font_dialog_error_has_user_message() {
        let error = choose_body_font_with(None, None, |_, _| {
            Err(ViewerError::font_dialog("CommDlgExtendedError: 0x0001"))
        })
        .expect_err("font dialog failure");

        assert_eq!(
            error.user_message(),
            "MDLuma could not open the font selection dialog."
        );
        assert!(error.operator_diagnostic().contains("CommDlgExtendedError"));
    }

    #[test]
    fn font_dialog_works_with_no_initial_settings() {
        let result = choose_body_font_with(None, None, |_, _| {
            Ok(FontPickOutcome::Selected {
                family_name: "Arial".to_string(),
                point_size_tenths: 100,
            })
        })
        .expect("choose body font without initial");

        assert!(
            matches!(result, FontDialogResult::Selected(ref bfs) if bfs.family_name == "Arial" && bfs.point_size_tenths == 100)
        );
    }

    #[test]
    fn font_dialog_preserves_initial_settings_identity() {
        let initial = BodyFontSettings {
            family_name: "Yu Gothic UI".to_string(),
            point_size_tenths: 120,
        };
        let result = choose_body_font_with(Some(&initial), None, |owner, init| {
            assert!(owner.is_none());
            let s = init.unwrap();
            Ok(FontPickOutcome::Selected {
                family_name: s.family_name.clone(),
                point_size_tenths: s.point_size_tenths,
            })
        })
        .expect("preserve initial identity");

        match result {
            FontDialogResult::Selected(bfs) => {
                assert_eq!(bfs, initial);
            }
            FontDialogResult::Cancelled => panic!("should not be cancelled"),
        }
    }

    #[test]
    fn owner_handle_is_forwarded_to_picker() {
        let owner = std::ptr::dangling_mut::<core::ffi::c_void>();
        let result = choose_body_font_with(None, Some(owner), |forwarded_owner, _| {
            assert_eq!(forwarded_owner, Some(owner));
            Ok(FontPickOutcome::Cancelled)
        })
        .expect("forward owner");

        assert!(matches!(result, FontDialogResult::Cancelled));
    }
}
