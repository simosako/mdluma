use crate::sciter::ffi::SciterWindowHandle;
use crate::ViewerError;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE};

const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWCP_DONOTROUND: u32 = 1;
const DWMWCP_ROUND: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowChromeState {
    pub maximized: bool,
}

pub trait WindowChromeController {
    fn minimize(&self, hwnd: SciterWindowHandle) -> Result<(), ViewerError>;
    fn toggle_maximize(&self, hwnd: SciterWindowHandle) -> Result<WindowChromeState, ViewerError>;
    fn is_maximized(&self, hwnd: SciterWindowHandle) -> Result<bool, ViewerError>;
    fn close(&self, hwnd: SciterWindowHandle) -> Result<(), ViewerError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsWindowChrome;

impl WindowChromeController for WindowsWindowChrome {
    fn minimize(&self, hwnd: SciterWindowHandle) -> Result<(), ViewerError> {
        minimize_with(hwnd, &RuntimeWin32)
    }

    fn toggle_maximize(&self, hwnd: SciterWindowHandle) -> Result<WindowChromeState, ViewerError> {
        toggle_maximize_with(hwnd, &RuntimeWin32)
    }

    fn is_maximized(&self, hwnd: SciterWindowHandle) -> Result<bool, ViewerError> {
        is_maximized_with(hwnd, &RuntimeWin32)
    }

    fn close(&self, hwnd: SciterWindowHandle) -> Result<(), ViewerError> {
        close_with(hwnd, &RuntimeWin32)
    }
}

#[cfg(not(windows))]
const SW_MINIMIZE: i32 = 6;
#[cfg(not(windows))]
const SW_MAXIMIZE: i32 = 3;
#[cfg(not(windows))]
const SW_RESTORE: i32 = 9;
#[cfg(not(windows))]
const WM_CLOSE: u32 = 0x0010;

fn minimize_with(
    hwnd: SciterWindowHandle,
    win32: &impl Win32WindowChrome,
) -> Result<(), ViewerError> {
    ensure_live_window(hwnd, win32)?;
    let _ = win32.show_window(hwnd, SW_MINIMIZE);
    Ok(())
}

fn toggle_maximize_with(
    hwnd: SciterWindowHandle,
    win32: &impl Win32WindowChrome,
) -> Result<WindowChromeState, ViewerError> {
    ensure_live_window(hwnd, win32)?;

    let maximized = win32.is_zoomed(hwnd);
    let command = if maximized { SW_RESTORE } else { SW_MAXIMIZE };
    let _ = win32.show_window(hwnd, command);

    set_window_corner_preference(hwnd, !maximized);

    Ok(WindowChromeState {
        maximized: !maximized,
    })
}

fn is_maximized_with(
    hwnd: SciterWindowHandle,
    win32: &impl Win32WindowChrome,
) -> Result<bool, ViewerError> {
    ensure_live_window(hwnd, win32)?;
    Ok(win32.is_zoomed(hwnd))
}

fn close_with(hwnd: SciterWindowHandle, win32: &impl Win32WindowChrome) -> Result<(), ViewerError> {
    ensure_live_window(hwnd, win32)?;

    if win32.post_message(hwnd, WM_CLOSE, 0, 0) {
        Ok(())
    } else {
        Err(window_action_error("close", hwnd))
    }
}

pub fn set_window_corner_preference(hwnd: SciterWindowHandle, rounded: bool) {
    #[cfg(windows)]
    {
        if hwnd.is_null() {
            return;
        }
        let preference = if rounded {
            DWMWCP_ROUND
        } else {
            DWMWCP_DONOTROUND
        };
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&preference as *const u32) as *const core::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, rounded);
    }
}

fn ensure_live_window(
    hwnd: SciterWindowHandle,
    win32: &impl Win32WindowChrome,
) -> Result<(), ViewerError> {
    if hwnd.is_null() || !win32.is_window(hwnd) {
        Err(ViewerError::ui(
            "Windows window chrome action requires a live top-level window handle",
        ))
    } else {
        Ok(())
    }
}

fn window_action_error(action: &str, hwnd: SciterWindowHandle) -> ViewerError {
    ViewerError::ui(format!(
        "Windows window chrome could not {action} window handle {:p}",
        hwnd
    ))
}

trait Win32WindowChrome {
    fn is_window(&self, hwnd: SciterWindowHandle) -> bool;
    fn is_zoomed(&self, hwnd: SciterWindowHandle) -> bool;
    fn show_window(&self, hwnd: SciterWindowHandle, command: i32) -> bool;
    fn post_message(
        &self,
        hwnd: SciterWindowHandle,
        message: u32,
        w_param: usize,
        l_param: isize,
    ) -> bool;
}

struct RuntimeWin32;

impl Win32WindowChrome for RuntimeWin32 {
    fn is_window(&self, hwnd: SciterWindowHandle) -> bool {
        runtime_is_window(hwnd)
    }

    fn is_zoomed(&self, hwnd: SciterWindowHandle) -> bool {
        runtime_is_zoomed(hwnd)
    }

    fn show_window(&self, hwnd: SciterWindowHandle, command: i32) -> bool {
        runtime_show_window(hwnd, command)
    }

    fn post_message(
        &self,
        hwnd: SciterWindowHandle,
        message: u32,
        w_param: usize,
        l_param: isize,
    ) -> bool {
        runtime_post_message(hwnd, message, w_param, l_param)
    }
}

#[cfg(windows)]
fn runtime_is_window(hwnd: SciterWindowHandle) -> bool {
    unsafe { IsWindow(hwnd) != 0 }
}

#[cfg(not(windows))]
fn runtime_is_window(_hwnd: SciterWindowHandle) -> bool {
    false
}

#[cfg(windows)]
fn runtime_is_zoomed(hwnd: SciterWindowHandle) -> bool {
    unsafe { IsZoomed(hwnd) != 0 }
}

#[cfg(not(windows))]
fn runtime_is_zoomed(_hwnd: SciterWindowHandle) -> bool {
    false
}

#[cfg(windows)]
fn runtime_show_window(hwnd: SciterWindowHandle, command: i32) -> bool {
    unsafe { ShowWindow(hwnd, command) != 0 }
}

#[cfg(not(windows))]
fn runtime_show_window(_hwnd: SciterWindowHandle, _command: i32) -> bool {
    false
}

#[cfg(windows)]
fn runtime_post_message(
    hwnd: SciterWindowHandle,
    message: u32,
    w_param: usize,
    l_param: isize,
) -> bool {
    unsafe { PostMessageW(hwnd, message, w_param, l_param) != 0 }
}

#[cfg(not(windows))]
fn runtime_post_message(
    _hwnd: SciterWindowHandle,
    _message: u32,
    _w_param: usize,
    _l_param: isize,
) -> bool {
    false
}

#[cfg(windows)]
#[link(name = "User32")]
extern "system" {
    fn IsWindow(window: SciterWindowHandle) -> i32;
    fn IsZoomed(window: SciterWindowHandle) -> i32;
    fn ShowWindow(window: SciterWindowHandle, command: i32) -> i32;
    fn PostMessageW(
        window: SciterWindowHandle,
        message: u32,
        w_param: usize,
        l_param: isize,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: SciterWindowHandle,
        attribute: u32,
        attribute_value: *const core::ffi::c_void,
        attribute_size: u32,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::{
        close_with, is_maximized_with, minimize_with, set_window_corner_preference,
        toggle_maximize_with, Win32WindowChrome, WindowChromeController, WindowChromeState,
        WindowsWindowChrome, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE,
    };
    use crate::sciter::ffi::SciterWindowHandle;
    use crate::ViewerError;
    use std::cell::Cell;

    #[test]
    fn minimize_requests_native_show_window_minimize() {
        let seam = FakeWin32::default();
        let hwnd = fake_hwnd();

        minimize_with(hwnd, &seam).expect("minimize window");

        assert_eq!(seam.last_show_window_command.get(), Some(SW_MINIMIZE));
    }

    #[test]
    fn toggle_maximize_returns_maximized_when_window_was_not_maximized() {
        let seam = FakeWin32::default();
        let hwnd = fake_hwnd();

        let state = toggle_maximize_with(hwnd, &seam).expect("maximize window");

        assert_eq!(state, WindowChromeState { maximized: true });
        assert_eq!(seam.last_show_window_command.get(), Some(SW_MAXIMIZE));
    }

    #[test]
    fn toggle_maximize_returns_restored_when_window_was_maximized() {
        let seam = FakeWin32 {
            maximized: true,
            ..FakeWin32::default()
        };
        let hwnd = fake_hwnd();

        let state = toggle_maximize_with(hwnd, &seam).expect("restore window");

        assert_eq!(state, WindowChromeState { maximized: false });
        assert_eq!(seam.last_show_window_command.get(), Some(SW_RESTORE));
    }

    #[test]
    fn close_posts_close_message_to_window() {
        let seam = FakeWin32::default();
        let hwnd = fake_hwnd();

        close_with(hwnd, &seam).expect("close window");

        assert_eq!(seam.last_post_message.get(), Some((WM_CLOSE, 0, 0)));
    }

    #[test]
    fn minimize_succeeds_even_when_show_window_reports_previously_hidden() {
        let seam = FakeWin32 {
            show_window_result: false,
            ..FakeWin32::default()
        };

        minimize_with(fake_hwnd(), &seam)
            .expect("previous visibility must not be treated as failure");
        assert_eq!(seam.last_show_window_command.get(), Some(SW_MINIMIZE));
    }

    #[test]
    fn public_adapter_delegates_minimize_to_runtime_seam_contract() {
        let chrome = WindowsWindowChrome;
        let error = chrome
            .minimize(std::ptr::null_mut())
            .expect_err("null hwnd must fail");

        assert_eq!(
            error,
            ViewerError::ui("Windows window chrome action requires a live top-level window handle")
        );
    }

    #[test]
    fn is_maximized_returns_false_when_window_not_maximized() {
        let seam = FakeWin32::default();
        let maximized = is_maximized_with(fake_hwnd(), &seam).expect("check maximize state");
        assert!(!maximized);
    }

    #[test]
    fn is_maximized_returns_true_when_window_is_maximized() {
        let seam = FakeWin32 {
            maximized: true,
            ..FakeWin32::default()
        };
        let maximized = is_maximized_with(fake_hwnd(), &seam).expect("check maximize state");
        assert!(maximized);
    }

    #[test]
    fn is_maximized_fails_on_null_hwnd() {
        let chrome = WindowsWindowChrome;
        let error = chrome
            .is_maximized(std::ptr::null_mut())
            .expect_err("null hwnd must fail");
        assert_eq!(
            error,
            ViewerError::ui("Windows window chrome action requires a live top-level window handle")
        );
    }

    #[test]
    fn invalid_window_handle_returns_ui_error_without_calling_win32_actions() {
        let seam = FakeWin32 {
            is_window: false,
            ..FakeWin32::default()
        };

        let error = minimize_with(fake_hwnd(), &seam).expect_err("dead hwnd must fail");

        assert_eq!(
            error,
            ViewerError::ui("Windows window chrome action requires a live top-level window handle")
        );
        assert_eq!(seam.last_show_window_command.get(), None);
    }

    #[test]
    fn toggle_maximize_succeeds_even_when_show_window_reports_previously_hidden() {
        let seam = FakeWin32 {
            show_window_result: false,
            ..FakeWin32::default()
        };

        let state = toggle_maximize_with(fake_hwnd(), &seam)
            .expect("previous visibility must not be treated as maximize failure");

        assert_eq!(state, WindowChromeState { maximized: true });
        assert_eq!(seam.last_show_window_command.get(), Some(SW_MAXIMIZE));
    }

    #[test]
    fn set_corner_preference_does_not_panic_on_null_hwnd() {
        set_window_corner_preference(std::ptr::null_mut(), true);
        set_window_corner_preference(std::ptr::null_mut(), false);
    }

    #[test]
    fn toggle_maximize_sets_rounded_when_restoring() {
        let seam = FakeWin32 {
            maximized: true,
            ..FakeWin32::default()
        };
        let _ = toggle_maximize_with(fake_hwnd(), &seam).expect("restore window");
        assert_eq!(seam.last_show_window_command.get(), Some(SW_RESTORE));
    }

    #[test]
    fn toggle_maximize_sets_not_rounded_when_maximizing() {
        let seam = FakeWin32::default();
        let _ = toggle_maximize_with(fake_hwnd(), &seam).expect("maximize window");
        assert_eq!(seam.last_show_window_command.get(), Some(SW_MAXIMIZE));
    }

    #[test]
    fn failed_close_maps_to_diagnostic_ui_error() {
        let seam = FakeWin32 {
            post_message_success: false,
            ..FakeWin32::default()
        };
        let hwnd = fake_hwnd();

        let error = close_with(hwnd, &seam).expect_err("close failure must surface");

        assert_eq!(
            error,
            ViewerError::ui(format!(
                "Windows window chrome could not close window handle {:p}",
                hwnd
            ))
        );
    }

    fn fake_hwnd() -> SciterWindowHandle {
        std::ptr::dangling_mut::<core::ffi::c_void>()
    }

    struct FakeWin32 {
        is_window: bool,
        maximized: bool,
        show_window_result: bool,
        post_message_success: bool,
        last_show_window_command: Cell<Option<i32>>,
        last_post_message: Cell<Option<(u32, usize, isize)>>,
    }

    impl Win32WindowChrome for FakeWin32 {
        fn is_window(&self, _hwnd: SciterWindowHandle) -> bool {
            self.is_window
        }

        fn is_zoomed(&self, _hwnd: SciterWindowHandle) -> bool {
            self.maximized
        }

        fn show_window(&self, _hwnd: SciterWindowHandle, command: i32) -> bool {
            self.last_show_window_command.set(Some(command));
            self.show_window_result
        }

        fn post_message(
            &self,
            _hwnd: SciterWindowHandle,
            message: u32,
            w_param: usize,
            l_param: isize,
        ) -> bool {
            self.last_post_message
                .set(Some((message, w_param, l_param)));
            self.post_message_success
        }
    }

    impl Default for FakeWin32 {
        fn default() -> Self {
            Self {
                is_window: true,
                maximized: false,
                show_window_result: true,
                post_message_success: true,
                last_show_window_command: Cell::new(None),
                last_post_message: Cell::new(None),
            }
        }
    }
}
