use crate::sciter::ffi::SciterWindowHandle;
use crate::ViewerError;
use std::path::PathBuf;

pub trait FileDialog {
    fn pick_markdown_file(
        &self,
        owner: Option<SciterWindowHandle>,
    ) -> Result<OpenFileResult, ViewerError>;

    fn pick_external_editor_file(
        &self,
        owner: Option<SciterWindowHandle>,
    ) -> Result<OpenFileResult, ViewerError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenFileResult {
    Selected(PathBuf),
    Cancelled,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsFileDialog;

impl FileDialog for WindowsFileDialog {
    fn pick_markdown_file(
        &self,
        owner: Option<SciterWindowHandle>,
    ) -> Result<OpenFileResult, ViewerError> {
        pick_markdown_file_with(owner, runtime_pick_markdown_file)
    }

    fn pick_external_editor_file(
        &self,
        owner: Option<SciterWindowHandle>,
    ) -> Result<OpenFileResult, ViewerError> {
        pick_markdown_file_with(owner, runtime_pick_external_editor_file)
    }
}

fn pick_markdown_file_with(
    owner: Option<SciterWindowHandle>,
    picker: impl FnOnce(Option<SciterWindowHandle>) -> Result<Option<PathBuf>, ViewerError>,
) -> Result<OpenFileResult, ViewerError> {
    match picker(owner)? {
        Some(path) => Ok(OpenFileResult::Selected(path)),
        None => Ok(OpenFileResult::Cancelled),
    }
}

#[cfg(windows)]
fn runtime_pick_markdown_file(
    owner: Option<SciterWindowHandle>,
) -> Result<Option<PathBuf>, ViewerError> {
    pick_open_file(
        owner,
        "Markdown Files (*.md;*.markdown)\0*.md;*.markdown\0All Files (*.*)\0*.*\0\0",
        "Open Markdown file",
        "md",
    )
}

#[cfg(windows)]
fn runtime_pick_external_editor_file(
    owner: Option<SciterWindowHandle>,
) -> Result<Option<PathBuf>, ViewerError> {
    pick_open_file(
        owner,
        "Executable Files (*.exe)\0*.exe\0All Files (*.*)\0*.*\0\0",
        "Select External Editor",
        "exe",
    )
}

#[cfg(windows)]
fn pick_open_file(
    owner: Option<SciterWindowHandle>,
    filter_str: &str,
    title: &str,
    default_ext: &str,
) -> Result<Option<PathBuf>, ViewerError> {
    use std::ffi::OsString;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, IsWindow, GA_ROOT};

    #[repr(C)]
    struct OpenFileNameW {
        l_struct_size: u32,
        hwnd_owner: *mut core::ffi::c_void,
        h_instance: *mut core::ffi::c_void,
        lpstr_filter: *const u16,
        lpstr_custom_filter: *mut u16,
        n_max_cust_filter: u32,
        n_filter_index: u32,
        lpstr_file: *mut u16,
        n_max_file: u32,
        lpstr_file_title: *mut u16,
        n_max_file_title: u32,
        lpstr_initial_dir: *const u16,
        lpstr_title: *const u16,
        flags: u32,
        n_file_offset: u16,
        n_file_extension: u16,
        lpstr_def_ext: *const u16,
        l_cust_data: isize,
        lpfn_hook: *mut core::ffi::c_void,
        lp_template_name: *const u16,
        pv_reserved: *mut core::ffi::c_void,
        dw_reserved: u32,
        flags_ex: u32,
    }

    #[link(name = "Comdlg32")]
    extern "system" {
        fn GetOpenFileNameW(ofn: *mut OpenFileNameW) -> i32;
        fn CommDlgExtendedError() -> u32;
    }

    const OFN_EXPLORER: u32 = 0x0008_0000;
    const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
    const OFN_HIDEREADONLY: u32 = 0x0000_0004;
    const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
    const OFN_NOCHANGEDIR: u32 = 0x0000_0008;

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

    let mut buffer = vec![0u16; 4096];
    let filter = widestr(filter_str);
    let title = widestr(title);
    let default_ext = widestr(default_ext);

    let mut dialog: OpenFileNameW = unsafe { zeroed() };
    dialog.l_struct_size = size_of::<OpenFileNameW>() as u32;
    dialog.lpstr_filter = filter.as_ptr();
    dialog.lpstr_file = buffer.as_mut_ptr();
    dialog.n_max_file = buffer.len() as u32;
    dialog.lpstr_title = title.as_ptr();
    dialog.lpstr_def_ext = default_ext.as_ptr();
    dialog.flags =
        OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_HIDEREADONLY | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    dialog.hwnd_owner = null_mut();
    dialog.h_instance = null_mut();
    dialog.lpstr_custom_filter = null_mut();
    dialog.lpstr_file_title = null_mut();
    dialog.lpstr_initial_dir = null();
    dialog.lp_template_name = null();
    dialog.pv_reserved = null_mut();
    dialog.hwnd_owner = owner.unwrap_or(null_mut());

    let selected = unsafe { GetOpenFileNameW(&mut dialog) };
    if selected != 0 {
        let length = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        let path = OsString::from_wide(&buffer[..length]);
        return Ok(Some(PathBuf::from(path)));
    }

    let error = unsafe { CommDlgExtendedError() };
    if error == 0 {
        Ok(None)
    } else {
        Err(ViewerError::file_dialog(format!(
            "Windows open file dialog failed with code 0x{error:04x}"
        )))
    }
}

#[cfg(not(windows))]
fn runtime_pick_markdown_file(
    _owner: Option<SciterWindowHandle>,
) -> Result<Option<PathBuf>, ViewerError> {
    Err(ViewerError::file_dialog(
        "Windows file dialog is unavailable on this platform",
    ))
}

#[cfg(not(windows))]
fn runtime_pick_external_editor_file(
    _owner: Option<SciterWindowHandle>,
) -> Result<Option<PathBuf>, ViewerError> {
    Err(ViewerError::file_dialog(
        "Windows file dialog is unavailable on this platform",
    ))
}

#[cfg(windows)]
fn widestr(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

#[cfg(test)]
mod tests {
    use super::{pick_markdown_file_with, FileDialog, OpenFileResult, WindowsFileDialog};
    use crate::ViewerError;
    use std::path::{Path, PathBuf};

    #[test]
    fn selected_path_is_returned_as_selected_outcome() {
        let result =
            pick_markdown_file_with(None, |_| Ok(Some(PathBuf::from(r"C:\docs\guide.md"))))
                .expect("select markdown file");

        assert_eq!(
            result,
            OpenFileResult::Selected(PathBuf::from(r"C:\docs\guide.md"))
        );
    }

    #[test]
    fn cancel_is_reported_without_treating_it_as_error() {
        let result = pick_markdown_file_with(None, |_| Ok(None)).expect("cancel dialog");

        assert_eq!(result, OpenFileResult::Cancelled);
    }

    #[test]
    fn picker_error_is_mapped_to_file_dialog_error() {
        let error = pick_markdown_file_with(None, |_| {
            Err(ViewerError::file_dialog("native dialog failed"))
        })
        .expect_err("propagate dialog error");

        assert_eq!(error, ViewerError::file_dialog("native dialog failed"));
    }

    #[test]
    fn public_contract_only_exposes_single_selection_or_cancel() {
        let dialog = WindowsFileDialog;
        let picker: &dyn FileDialog = &dialog;

        let selected = OpenFileResult::Selected(PathBuf::from(r"C:\docs\guide.md"));
        let cancelled = OpenFileResult::Cancelled;

        assert!(matches!(selected, OpenFileResult::Selected(_)));
        assert!(matches!(cancelled, OpenFileResult::Cancelled));
        assert_eq!(
            std::mem::size_of_val(&picker),
            std::mem::size_of::<&dyn FileDialog>()
        );
    }

    #[test]
    fn dialog_error_has_user_facing_message_for_failed_picker() {
        let error =
            pick_markdown_file_with(None, |_| Err(ViewerError::file_dialog("native failure")))
                .expect_err("file dialog failure");

        assert_eq!(
            error.user_message(),
            "MDLuma could not open the file selection dialog."
        );
        assert!(error.operator_diagnostic().contains("native failure"));
    }

    #[test]
    fn selected_result_preserves_local_markdown_path_identity() {
        let path = Path::new(r"C:\docs\notes.markdown");
        let result = pick_markdown_file_with(None, |_| Ok(Some(path.to_path_buf())))
            .expect("select markdown path");

        match result {
            OpenFileResult::Selected(selected) => assert_eq!(selected, path),
            OpenFileResult::Cancelled => panic!("selection should not be cancelled"),
        }
    }

    #[test]
    fn owner_handle_is_forwarded_to_picker() {
        let owner = std::ptr::dangling_mut::<core::ffi::c_void>();
        let result = pick_markdown_file_with(Some(owner), |forwarded_owner| {
            assert_eq!(forwarded_owner, Some(owner));
            Ok(None)
        })
        .expect("forward owner handle");

        assert_eq!(result, OpenFileResult::Cancelled);
    }

    #[test]
    fn external_editor_selected_path_is_returned_as_selected_outcome() {
        let result =
            pick_markdown_file_with(None, |_| Ok(Some(PathBuf::from(r"C:\tools\editor.exe"))))
                .expect("select external editor file");

        assert_eq!(
            result,
            OpenFileResult::Selected(PathBuf::from(r"C:\tools\editor.exe"))
        );
    }

    #[test]
    fn external_editor_cancel_is_reported_without_treating_it_as_error() {
        let result = pick_markdown_file_with(None, |_| Ok(None)).expect("cancel editor dialog");

        assert_eq!(result, OpenFileResult::Cancelled);
    }

    #[test]
    fn external_editor_picker_error_is_mapped_to_file_dialog_error() {
        let error = pick_markdown_file_with(None, |_| {
            Err(ViewerError::file_dialog("native editor dialog failed"))
        })
        .expect_err("propagate editor dialog error");

        assert_eq!(
            error,
            ViewerError::file_dialog("native editor dialog failed")
        );
    }

    #[test]
    fn external_editor_public_contract_only_exposes_single_selection_or_cancel() {
        let dialog = WindowsFileDialog;
        let picker: &dyn FileDialog = &dialog;

        let selected = OpenFileResult::Selected(PathBuf::from(r"C:\tools\editor.exe"));
        let cancelled = OpenFileResult::Cancelled;

        assert!(matches!(selected, OpenFileResult::Selected(_)));
        assert!(matches!(cancelled, OpenFileResult::Cancelled));
        assert_eq!(
            std::mem::size_of_val(&picker),
            std::mem::size_of::<&dyn FileDialog>()
        );
    }

    #[test]
    fn external_editor_owner_handle_is_forwarded_to_picker() {
        let owner = std::ptr::dangling_mut::<core::ffi::c_void>();
        let result = pick_markdown_file_with(Some(owner), |forwarded_owner| {
            assert_eq!(forwarded_owner, Some(owner));
            Ok(None)
        })
        .expect("forward owner handle for editor picker");

        assert_eq!(result, OpenFileResult::Cancelled);
    }

    #[test]
    fn external_editor_dialog_error_has_user_facing_message() {
        let error = pick_markdown_file_with(None, |_| {
            Err(ViewerError::file_dialog("editor dialog failure"))
        })
        .expect_err("editor file dialog failure");

        assert_eq!(
            error.user_message(),
            "MDLuma could not open the file selection dialog."
        );
        assert!(error
            .operator_diagnostic()
            .contains("editor dialog failure"));
    }
}
