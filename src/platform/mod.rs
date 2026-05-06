mod font_dialog;
mod windows_file_dialog;
mod windows_font_dialog;
mod windows_window_chrome;

pub use font_dialog::{FontDialog, FontDialogResult};
pub use windows_file_dialog::{FileDialog, OpenFileResult, WindowsFileDialog};
pub use windows_font_dialog::WindowsFontDialog;
pub use windows_window_chrome::{set_window_corner_preference, WindowChromeController, WindowChromeState, WindowsWindowChrome};

pub fn open_url_in_browser(url: &str) {
    crate::platform::windows_browser::open_url_in_browser(url);
}

#[cfg(windows)]
mod windows_browser;
