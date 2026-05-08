use crate::sciter::runtime::{SciterRuntimeError, SciterVersion, SCITER_DLL_NAME};
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::{Mutex, OnceLock};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, GWLP_WNDPROC, GWL_STYLE, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, SW_SHOW, SW_SHOWMINIMIZED, WM_APP, WM_CLOSE,
    WM_DROPFILES, WM_NCRBUTTONUP, WM_WINDOWPOSCHANGING, WS_CAPTION, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_SYSMENU,
};

#[cfg(windows)]
mod generated {
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(dead_code)]
    include!("generated_sciter_bindings.rs");
}

#[cfg(windows)]
pub use generated::{SCITER_VERSION_0, SCITER_VERSION_1};

pub type SciterWindowHandle = *mut core::ffi::c_void;
pub type SciterElementHandle = *mut core::ffi::c_void;
type SciterLpcwstrReceiver =
    unsafe extern "system" fn(value: *const u16, value_length: u32, param: *mut core::ffi::c_void);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SciterRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct WindowPoint {
    x: i32,
    y: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct WindowPlacement {
    length: u32,
    flags: u32,
    show_cmd: u32,
    min_position: WindowPoint,
    max_position: WindowPoint,
    normal_position: SciterRect,
}

pub type SciterCallback = unsafe extern "system" fn() -> u32;
pub type SciterElementEventProc = unsafe extern "system" fn(
    tag: *mut core::ffi::c_void,
    element: SciterElementHandle,
    event_group: u32,
    params: *mut core::ffi::c_void,
) -> i32;

pub const HANDLE_BEHAVIOR_EVENT: u32 = 0x0100;
pub const HANDLE_EXCHANGE: u32 = 0x1000;
pub const HANDLE_SCRIPTING_METHOD_CALL: u32 = 0x0400;
pub const BEHAVIOR_EVENT_CUSTOM: u32 = 0x00f0;
pub const X_WILL_ACCEPT_DROP: u32 = 7;
pub const X_DROP: u32 = 3;
const SCDOM_OK: i32 = 0;
const SCITER_SET_SCRIPT_RUNTIME_FEATURES: u32 = 8;
#[cfg(debug_assertions)]
const SCITER_SET_DEBUG_MODE: u32 = 10;
const SCITER_SET_INIT_SCRIPT: u32 = 13;
const SCRIPT_RUNTIME_VIEWER_FLAGS: usize = 0;
const SW_RESIZEABLE: u32 = 1 << 2;
const SW_MAIN: u32 = 1 << 7;
#[cfg(test)]
const SW_TITLEBAR: u32 = 1 << 5;
#[cfg(test)]
const SW_CONTROLS: u32 = 1 << 4;
#[cfg(windows)]
const WM_DL_DEFERRED_LOAD: u32 = WM_APP + 0x100;
#[cfg(windows)]
const HTCAPTION: usize = 2;

const EXTENDED_FRAME_INIT_SCRIPT: &[u8] = b"Window.this.frameType = \"extended\";\0";

#[cfg(windows)]
const SCITER_APP_INIT: u32 = 2;
#[cfg(windows)]
const SCITER_APP_LOOP: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SciterValue {
    pub value_type: u32,
    pub units: u32,
    pub data: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BehaviorEventParams {
    pub cmd: u32,
    pub target: SciterElementHandle,
    pub source: SciterElementHandle,
    pub reason: usize,
    pub data: SciterValue,
    pub name: *const u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ExchangeParams {
    pub cmd: u32,
    pub target: SciterElementHandle,
    pub source: SciterElementHandle,
    pub pos_x: i32,
    pub pos_y: i32,
    pub pos_view_x: i32,
    pub pos_view_y: i32,
    pub mode: u32,
    pub data: SciterValue,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ScriptingMethodParams {
    pub name: *const i8,
    pub argv: *const SciterValue,
    pub argc: u32,
    pub result: SciterValue,
}

#[derive(Debug)]
pub struct SciterApi {
    #[cfg(windows)]
    #[allow(dead_code)]
    library: Option<WindowsLibrary>,
    sciter_version: SciterVersionFn,
    #[allow(dead_code)]
    pub sciter_create_window: SciterCreateWindowFn,
    #[allow(dead_code)]
    pub sciter_load_html: SciterLoadHtmlFn,
    sciter_set_option: SciterSetOptionFn,
    #[cfg(windows)]
    sciter_get_attribute_by_name_cb: SciterGetAttributeByNameCbFn,
    #[cfg(windows)]
    sciter_get_parent_element: SciterGetParentElementFn,
    #[cfg(windows)]
    sciter_get_root_element: SciterGetRootElementFn,
    #[cfg(windows)]
    sciter_eval_element_script: SciterEvalElementScriptFn,
    #[cfg(windows)]
    sciter_use_element: SciterElementRefFn,
    #[cfg(windows)]
    sciter_unuse_element: SciterElementRefFn,
    sciter_window_attach_event_handler: SciterWindowAttachEventHandlerFn,
    sciter_window_detach_event_handler: SciterWindowDetachEventHandlerFn,
    sciter_value_type: SciterValueTypeFn,
    sciter_value_string_data: SciterValueStringDataFn,
    #[cfg(windows)]
    sciter_value_init: SciterValueInitFn,
    #[cfg(windows)]
    sciter_value_clear: SciterValueClearFn,
    #[cfg(windows)]
    sciter_value_string_data_set: SciterValueStringDataSetFn,
    #[cfg(windows)]
    sciter_value_elements_count: SciterValueElementsCountFn,
    #[cfg(windows)]
    sciter_value_nth_element_value: SciterValueNthElementValueFn,
    #[cfg(windows)]
    sciter_value_get_value_of_key: SciterValueGetValueOfKeyFn,
    #[cfg(windows)]
    sciter_update_window: SciterUpdateWindowFn,
    #[cfg(windows)]
    sciter_exec: SciterExecFn,
    #[cfg(windows)]
    sciter_setup_debug_output: SciterSetupDebugOutputFn,
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct SciterApiBindings {
    sciter_version: SciterVersionFn,
    sciter_create_window: SciterCreateWindowFn,
    sciter_load_html: SciterLoadHtmlFn,
    sciter_set_option: SciterSetOptionFn,
    sciter_get_attribute_by_name_cb: SciterGetAttributeByNameCbFn,
    sciter_get_parent_element: SciterGetParentElementFn,
    sciter_get_root_element: SciterGetRootElementFn,
    sciter_eval_element_script: SciterEvalElementScriptFn,
    sciter_use_element: SciterElementRefFn,
    sciter_unuse_element: SciterElementRefFn,
    sciter_window_attach_event_handler: SciterWindowAttachEventHandlerFn,
    sciter_window_detach_event_handler: SciterWindowDetachEventHandlerFn,
    sciter_value_type: SciterValueTypeFn,
    sciter_value_string_data: SciterValueStringDataFn,
    sciter_value_init: SciterValueInitFn,
    sciter_value_clear: SciterValueClearFn,
    sciter_value_string_data_set: SciterValueStringDataSetFn,
    sciter_value_elements_count: SciterValueElementsCountFn,
    sciter_value_nth_element_value: SciterValueNthElementValueFn,
    sciter_value_get_value_of_key: SciterValueGetValueOfKeyFn,
    sciter_update_window: SciterUpdateWindowFn,
    sciter_exec: SciterExecFn,
    sciter_setup_debug_output: SciterSetupDebugOutputFn,
}

type SciterVersionFn = unsafe extern "C" fn(major: u32) -> u32;
pub type SciterCreateWindowFn = unsafe extern "C" fn(
    creation_flags: u32,
    frame: *const core::ffi::c_void,
    delegate: Option<SciterCallback>,
    delegate_param: *mut core::ffi::c_void,
    parent: SciterWindowHandle,
) -> SciterWindowHandle;
pub type SciterLoadHtmlFn = unsafe extern "C" fn(
    hwnd: SciterWindowHandle,
    html: *const u8,
    html_length: u32,
    base_url: *const u16,
) -> i32;
type SciterWindowAttachEventHandlerFn = unsafe extern "C" fn(
    hwnd: SciterWindowHandle,
    event_proc: SciterElementEventProc,
    tag: *mut core::ffi::c_void,
    subscription: u32,
) -> i32;
type SciterWindowDetachEventHandlerFn = unsafe extern "C" fn(
    hwnd: SciterWindowHandle,
    event_proc: SciterElementEventProc,
    tag: *mut core::ffi::c_void,
) -> i32;
type SciterSetOptionFn =
    unsafe extern "C" fn(hwnd: SciterWindowHandle, option: u32, value: usize) -> i32;
#[cfg(windows)]
type SciterDebugOutputProc = unsafe extern "C" fn(
    param: *mut core::ffi::c_void,
    subsystem: u32,
    severity: u32,
    text: *const u16,
    text_length: u32,
);
#[cfg(windows)]
type SciterSetupDebugOutputFn = unsafe extern "C" fn(
    hwnd_or_null: SciterWindowHandle,
    param: *mut core::ffi::c_void,
    output: Option<SciterDebugOutputProc>,
);
#[cfg(windows)]
type SciterUpdateWindowFn = unsafe extern "C" fn(hwnd: SciterWindowHandle);
#[cfg(windows)]
type SciterExecFn = unsafe extern "C" fn(cmd: u32, p1: usize, p2: usize) -> isize;
#[cfg(windows)]
type SciterGetAttributeByNameCbFn = unsafe extern "C" fn(
    element: SciterElementHandle,
    name: *const i8,
    receiver: SciterLpcwstrReceiver,
    receiver_param: *mut core::ffi::c_void,
) -> i32;
#[cfg(windows)]
type SciterGetParentElementFn =
    unsafe extern "C" fn(element: SciterElementHandle, parent: *mut SciterElementHandle) -> i32;
#[cfg(windows)]
type SciterGetRootElementFn =
    unsafe extern "C" fn(window: SciterWindowHandle, root: *mut SciterElementHandle) -> i32;
#[cfg(windows)]
type SciterEvalElementScriptFn = unsafe extern "C" fn(
    element: SciterElementHandle,
    script: *const u16,
    script_length: u32,
    retval: *mut SciterValue,
) -> i32;
#[cfg(windows)]
type SciterElementRefFn = unsafe extern "C" fn(element: SciterElementHandle) -> i32;
type SciterValueTypeFn =
    unsafe extern "C" fn(pval: *const SciterValue, p_type: *mut u32, p_units: *mut u32) -> u32;
type SciterValueStringDataFn = unsafe extern "C" fn(
    pval: *const SciterValue,
    p_chars: *mut *const u16,
    p_num_chars: *mut u32,
) -> u32;
#[cfg(windows)]
type SciterValueInitFn = unsafe extern "C" fn(pval: *mut SciterValue) -> u32;
#[cfg(windows)]
type SciterValueClearFn = SciterValueInitFn;
#[cfg(windows)]
type SciterValueStringDataSetFn = unsafe extern "C" fn(
    pval: *mut SciterValue,
    chars: *const u16,
    num_chars: u32,
    units: u32,
) -> u32;
#[cfg(windows)]
type SciterValueElementsCountFn =
    unsafe extern "C" fn(pval: *const SciterValue, pn: *mut i32) -> u32;
#[cfg(windows)]
type SciterValueNthElementValueFn =
    unsafe extern "C" fn(pval: *const SciterValue, n: i32, pretval: *mut SciterValue) -> u32;
#[cfg(windows)]
type SciterValueGetValueOfKeyFn = unsafe extern "C" fn(
    pval: *const SciterValue,
    pkey: *const SciterValue,
    pretval: *mut SciterValue,
) -> u32;

impl SciterApi {
    pub fn load(dll_path: &Path) -> Result<Self, SciterRuntimeError> {
        #[cfg(windows)]
        {
            let library = WindowsLibrary::load(dll_path)?;
            let bindings = unsafe { Self::load_bindings_from_api_table(&library) }?;
            Ok(Self::from_bindings(library, bindings))
        }

        #[cfg(not(windows))]
        {
            let _ = dll_path;
            Err(SciterRuntimeError::ApiUnavailable {
                message: "Sciter runtime is only available on Windows in this build".to_string(),
            })
        }
    }

    pub fn version(&self) -> Result<SciterVersion, SciterRuntimeError> {
        let major = unsafe { (self.sciter_version)(0u32) };
        let minor = unsafe { (self.sciter_version)(1u32) };
        let patch = unsafe { (self.sciter_version)(2u32) };
        let build = unsafe { (self.sciter_version)(3u32) };

        Ok(SciterVersion {
            major,
            minor,
            patch,
            build,
        })
    }

    pub fn sciter_value_type(&self, value: &SciterValue) -> Option<(u32, u32)> {
        let mut type_out: u32 = 0;
        let mut units_out: u32 = 0;
        let result = unsafe {
            (self.sciter_value_type)(value as *const SciterValue, &mut type_out, &mut units_out)
        };
        if result == 0 {
            Some((type_out, units_out))
        } else {
            None
        }
    }

    pub fn sciter_value_string_data(&self, value: &SciterValue) -> Option<String> {
        let mut chars: *const u16 = std::ptr::null();
        let mut num_chars: u32 = 0;
        let result = unsafe {
            (self.sciter_value_string_data)(value as *const SciterValue, &mut chars, &mut num_chars)
        };
        if result != 0 || chars.is_null() {
            return None;
        }
        let slice = unsafe { std::slice::from_raw_parts(chars, num_chars as usize) };
        String::from_utf16(slice).ok()
    }

    #[cfg(windows)]
    pub(crate) fn extract_exchange_drop_paths(&self, data: &SciterValue) -> Vec<PathBuf> {
        let Some(file_val) = self.get_map_value(data, "file") else {
            return Vec::new();
        };

        if let Some(s) = sciter_value_to_string(self, &file_val) {
            return vec![sciter_file_uri_to_path(&s)];
        }

        let file_count = self.value_element_count(&file_val).unwrap_or(0);
        if file_count == 0 {
            return Vec::new();
        }

        let mut paths = Vec::with_capacity(file_count as usize);
        for i in 0..file_count {
            if let Some(elem) = self.value_nth_element(&file_val, i) {
                if let Some(s) = sciter_value_to_string(self, &elem) {
                    paths.push(sciter_file_uri_to_path(&s));
                }
            }
        }
        paths
    }

    #[cfg(windows)]
    fn get_map_value(&self, map: &SciterValue, key: &str) -> Option<SciterValue> {
        let mut key_val = SciterValue::default();
        unsafe {
            let init_result = (self.sciter_value_init)(&mut key_val);
            if init_result != 0 {
                return None;
            }
            let wide: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
            let set_result = (self.sciter_value_string_data_set)(
                &mut key_val,
                wide.as_ptr(),
                (wide.len() - 1) as u32,
                0,
            );
            if set_result != 0 {
                (self.sciter_value_clear)(&mut key_val);
                return None;
            }
            let mut result = SciterValue::default();
            (self.sciter_value_init)(&mut result);
            let get_result = (self.sciter_value_get_value_of_key)(map, &key_val, &mut result);
            (self.sciter_value_clear)(&mut key_val);
            if get_result != 0 {
                (self.sciter_value_clear)(&mut result);
                return None;
            }
            let (vt, _) = self.sciter_value_type(&result).unwrap_or((0, 0));
            if vt == 0 {
                (self.sciter_value_clear)(&mut result);
                return None;
            }
            Some(result)
        }
    }

    #[cfg(windows)]
    fn value_element_count(&self, val: &SciterValue) -> Option<i32> {
        let mut count: i32 = 0;
        let result = unsafe { (self.sciter_value_elements_count)(val, &mut count) };
        if result == 0 {
            Some(count)
        } else {
            None
        }
    }

    #[cfg(windows)]
    fn value_nth_element(&self, val: &SciterValue, n: i32) -> Option<SciterValue> {
        let mut elem = SciterValue::default();
        unsafe {
            (self.sciter_value_init)(&mut elem);
            let result = (self.sciter_value_nth_element_value)(val, n, &mut elem);
            if result != 0 {
                (self.sciter_value_clear)(&mut elem);
                return None;
            }
            Some(elem)
        }
    }

    pub(crate) fn create_window(
        &self,
        saved_geometry: Option<&crate::settings::WindowGeometry>,
    ) -> Result<SciterWindowHandle, SciterRuntimeError> {
        let (cascade_left, cascade_top) = read_window_cascade_offset();
        let mut frame = if let Some(geo) = saved_geometry {
            SciterRect {
                left: geo.left + cascade_left,
                top: geo.top + cascade_top,
                right: geo.right + cascade_left,
                bottom: geo.bottom + cascade_top,
            }
        } else {
            SciterRect {
                left: 100 + cascade_left,
                top: 100 + cascade_top,
                right: 1180 + cascade_left,
                bottom: 820 + cascade_top,
            }
        };
        if !is_on_screen_rect(&frame) {
            frame = SciterRect {
                left: 100 + cascade_left,
                top: 100 + cascade_top,
                right: 1180 + cascade_left,
                bottom: 820 + cascade_top,
            };
        }
        let creation_flags = viewer_window_creation_flags();
        self.configure_script_runtime_features()?;
        if should_install_init_script() {
            self.install_window_init_script()?;
        } else {
            crate::debug_log!(
                "Skipping SCITER_SET_INIT_SCRIPT due to MDLUMA_SCITER_DISABLE_INIT_SCRIPT"
            );
        }

        let handle = unsafe {
            (self.sciter_create_window)(
                creation_flags,
                (&mut frame as *mut SciterRect).cast(),
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        if handle.is_null() {
            Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterCreateWindow returned a null handle".to_string(),
            })
        } else {
            self.setup_debug_output(handle);
            #[cfg(debug_assertions)]
            self.enable_debug_mode(handle)?;
            self.remove_native_titlebar(handle)?;
            crate::platform::set_window_corner_preference(handle, true);
            Ok(handle)
        }
    }

    #[cfg(windows)]
    fn setup_debug_output(&self, window: SciterWindowHandle) {
        unsafe {
            (self.sciter_setup_debug_output)(
                window,
                std::ptr::null_mut(),
                Some(sciter_debug_output_proc),
            );
        }
        crate::debug_log!("SciterSetupDebugOutput installed");
    }

    #[cfg(not(windows))]
    fn setup_debug_output(&self, _window: SciterWindowHandle) {}

    #[cfg(debug_assertions)]
    fn enable_debug_mode(&self, window: SciterWindowHandle) -> Result<(), SciterRuntimeError> {
        let configured = unsafe { (self.sciter_set_option)(window, SCITER_SET_DEBUG_MODE, 1) };

        if configured != 0 {
            Ok(())
        } else {
            Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterSetOption failed to enable script debug mode".to_string(),
            })
        }
    }

    fn configure_script_runtime_features(&self) -> Result<(), SciterRuntimeError> {
        let configured = unsafe {
            (self.sciter_set_option)(
                std::ptr::null_mut(),
                SCITER_SET_SCRIPT_RUNTIME_FEATURES,
                SCRIPT_RUNTIME_VIEWER_FLAGS,
            )
        };

        if configured != 0 {
            Ok(())
        } else {
            Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterSetOption failed to configure script runtime features".to_string(),
            })
        }
    }

    pub(crate) fn load_html(
        &self,
        window: SciterWindowHandle,
        html: &str,
    ) -> Result<(), SciterRuntimeError> {
        if window.is_null() {
            return Err(SciterRuntimeError::ApiUnavailable {
                message: "Sciter window handle is null".to_string(),
            });
        }

        let loaded = unsafe {
            (self.sciter_load_html)(window, html.as_ptr(), html.len() as u32, std::ptr::null())
        };

        if loaded == 0 {
            return Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterLoadHtml returned failure status".to_string(),
            });
        }

        Ok(())
    }

    pub(crate) fn show_window(&self, window: SciterWindowHandle) {
        #[cfg(windows)]
        if self.library.is_some() {
            unsafe {
                ShowWindow(window, SW_SHOW);
            }
        }

        #[cfg(not(windows))]
        {
            let _ = window;
        }
    }

    #[cfg(windows)]
    pub(crate) fn update_window(&self, window: SciterWindowHandle) {
        if self.library.is_some() {
            unsafe {
                (self.sciter_update_window)(window);
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn sciter_exec(&self) -> SciterExecFn {
        self.sciter_exec
    }

    #[cfg(windows)]
    pub(crate) fn setup_deferred_load(&self, window: SciterWindowHandle) {
        if self.library.is_none() {
            return;
        }
        unsafe {
            let prev = SetWindowLongPtrW(
                window,
                GWLP_WNDPROC,
                deferred_load_wnd_proc as *const () as isize,
            );
            if prev == 0 {
                crate::debug_log!(
                    "deferred load subclass: SetWindowLongPtrW failed, skipping install"
                );
                return;
            }
            *std::ptr::addr_of_mut!(DL_ORIGINAL_PROC) =
                Some(std::mem::transmute::<isize, WndProcFn>(prev));
            *std::ptr::addr_of_mut!(DL_GET_ROOT_ELEMENT_FN) = Some(self.sciter_get_root_element);
            *std::ptr::addr_of_mut!(DL_EVAL_ELEMENT_SCRIPT_FN) = Some(self.sciter_eval_element_script);
        }
        unsafe {
            DragAcceptFiles(window, 1);
        }
    }

    #[cfg(windows)]
    pub(crate) fn should_defer_load(&self) -> bool {
        self.library.is_some()
    }

    #[cfg(windows)]
    pub(crate) fn defer_load_html(
        &self,
        window: SciterWindowHandle,
        html: &str,
    ) -> Result<(), SciterRuntimeError> {
        unsafe {
            *std::ptr::addr_of_mut!(DL_PENDING_HTML) = Some(PendingDeferredLoad {
                load_fn: self.sciter_load_html,
                html: html.to_string(),
                placement: capture_window_placement(window),
            });
            let posted = PostMessageW(window, WM_DL_DEFERRED_LOAD, 0, 0);
            if posted == 0 {
                return Err(SciterRuntimeError::ApiUnavailable {
                    message: "PostMessageW(WM_DL_DEFERRED_LOAD) failed".to_string(),
                });
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn init_app(&self) {
        unsafe {
            (self.sciter_exec)(SCITER_APP_INIT, 0, 0);
        }
    }

    #[cfg(windows)]
    pub(crate) fn run_app_loop(&self) -> isize {
        unsafe { (self.sciter_exec)(SCITER_APP_LOOP, 0, 0) }
    }

    #[cfg(windows)]
    pub(crate) fn restore_pending_window_placement(&self, window: SciterWindowHandle) {
        if self.library.is_none() {
            return;
        }

        unsafe {
            let pending =
                std::ptr::replace(std::ptr::addr_of_mut!(DL_PENDING_PLACEMENT_RESTORE), None);
            if let Some(placement) = pending {
                restore_window_placement(window, placement);
            }
        }
    }

    pub(crate) fn get_attribute_by_name(
        &self,
        element: SciterElementHandle,
        name: &str,
    ) -> Option<String> {
        #[cfg(windows)]
        {
            if element.is_null() {
                return None;
            }

            self.with_active_element(element, |api| {
                let mut storage = String::new();
                let mut name_bytes = name.as_bytes().to_vec();
                name_bytes.push(0);

                let status = unsafe {
                    (api.sciter_get_attribute_by_name_cb)(
                        element,
                        name_bytes.as_ptr() as *const i8,
                        store_utf16_string,
                        (&mut storage as *mut String).cast(),
                    )
                };

                if status == SCDOM_OK {
                    Some(storage)
                } else {
                    None
                }
            })
        }

        #[cfg(not(windows))]
        {
            let _ = element;
            let _ = name;
            None
        }
    }

    pub(crate) fn get_parent_element(
        &self,
        element: SciterElementHandle,
    ) -> Option<SciterElementHandle> {
        #[cfg(windows)]
        {
            if element.is_null() {
                return None;
            }

            self.with_active_element(element, |api| {
                let mut parent = std::ptr::null_mut();
                let status = unsafe { (api.sciter_get_parent_element)(element, &mut parent) };

                if status == SCDOM_OK && !parent.is_null() {
                    Some(parent)
                } else {
                    None
                }
            })
        }

        #[cfg(not(windows))]
        {
            let _ = element;
            None
        }
    }

    #[cfg(windows)]
    fn with_active_element<T>(
        &self,
        element: SciterElementHandle,
        f: impl FnOnce(&Self) -> Option<T>,
    ) -> Option<T> {
        let use_status = unsafe { (self.sciter_use_element)(element) };
        if use_status != SCDOM_OK {
            return None;
        }

        let result = f(self);
        unsafe { (self.sciter_unuse_element)(element) };
        result
    }

    fn install_window_init_script(&self) -> Result<(), SciterRuntimeError> {
        let configured = unsafe {
            (self.sciter_set_option)(
                std::ptr::null_mut(),
                SCITER_SET_INIT_SCRIPT,
                EXTENDED_FRAME_INIT_SCRIPT.as_ptr() as usize,
            )
        };

        if configured != 0 {
            Ok(())
        } else {
            Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterSetOption failed to install the extended-frame init script"
                    .to_string(),
            })
        }
    }

    fn remove_native_titlebar(&self, window: SciterWindowHandle) -> Result<(), SciterRuntimeError> {
        #[cfg(windows)]
        {
            if self.library.is_none() {
                return Ok(());
            }

            let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) };
            if style == 0 {
                return Err(SciterRuntimeError::ApiUnavailable {
                    message: "GetWindowLongPtrW returned a zero window style while removing the native titlebar"
                        .to_string(),
                });
            }

            let stripped_style = strip_native_titlebar_style(style);
            let _ = unsafe { SetWindowLongPtrW(window, GWL_STYLE, stripped_style) };

            let refreshed = unsafe {
                SetWindowPos(
                    window,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                )
            };

            if refreshed == 0 {
                return Err(SciterRuntimeError::ApiUnavailable {
                    message: "SetWindowPos failed while refreshing the native frame".to_string(),
                });
            }

            Ok(())
        }

        #[cfg(not(windows))]
        {
            let _ = window;
            Ok(())
        }
    }

    pub(crate) fn attach_window_event_handler(
        &self,
        window: SciterWindowHandle,
        event_proc: SciterElementEventProc,
        tag: *mut core::ffi::c_void,
        subscription: u32,
    ) -> Result<(), SciterRuntimeError> {
        let status = unsafe {
            (self.sciter_window_attach_event_handler)(window, event_proc, tag, subscription)
        };

        if status == SCDOM_OK {
            Ok(())
        } else {
            Err(SciterRuntimeError::ApiUnavailable {
                message: format!("SciterWindowAttachEventHandler failed with status {status}"),
            })
        }
    }

    pub(crate) fn detach_window_event_handler(
        &self,
        window: SciterWindowHandle,
        event_proc: SciterElementEventProc,
        tag: *mut core::ffi::c_void,
    ) -> Result<(), SciterRuntimeError> {
        let status = unsafe { (self.sciter_window_detach_event_handler)(window, event_proc, tag) };

        if status == SCDOM_OK {
            Ok(())
        } else {
            Err(SciterRuntimeError::ApiUnavailable {
                message: format!("SciterWindowDetachEventHandler failed with status {status}"),
            })
        }
    }

    #[cfg(windows)]
    pub(crate) fn eval_document_script(
        &self,
        window: SciterWindowHandle,
        script: &str,
    ) -> Result<(), SciterRuntimeError> {
        let mut root = std::ptr::null_mut();
        let root_status = unsafe { (self.sciter_get_root_element)(window, &mut root) };
        if root_status != SCDOM_OK || root.is_null() {
            return Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterGetRootElement failed".to_string(),
            });
        }

        let wide_script: Vec<u16> = script.encode_utf16().collect();
        let eval_status = unsafe {
            (self.sciter_eval_element_script)(
                root,
                wide_script.as_ptr(),
                wide_script.len() as u32,
                std::ptr::null_mut(),
            )
        };
        if eval_status == SCDOM_OK {
            Ok(())
        } else {
            Err(SciterRuntimeError::ApiUnavailable {
                message: format!(
                    "SciterEvalElementScript failed with status {eval_status}"
                ),
            })
        }
    }

    #[cfg(test)]
    pub(crate) fn for_tests(
        sciter_version: SciterVersionFn,
        sciter_create_window: SciterCreateWindowFn,
        sciter_load_html: SciterLoadHtmlFn,
        sciter_set_option: SciterSetOptionFn,
        sciter_value_type: SciterValueTypeFn,
        sciter_value_string_data: SciterValueStringDataFn,
    ) -> Self {
        Self {
            #[cfg(windows)]
            library: None,
            sciter_version,
            sciter_create_window,
            sciter_load_html,
            sciter_set_option,
            #[cfg(windows)]
            sciter_get_attribute_by_name_cb: fake_sciter_get_attribute_by_name_cb,
            #[cfg(windows)]
            sciter_get_parent_element: fake_sciter_get_parent_element,
            #[cfg(windows)]
            sciter_get_root_element: fake_sciter_get_root_element,
            #[cfg(windows)]
            sciter_eval_element_script: fake_sciter_eval_element_script,
            #[cfg(windows)]
            sciter_use_element: fake_sciter_element_ref,
            #[cfg(windows)]
            sciter_unuse_element: fake_sciter_element_ref,
            sciter_window_attach_event_handler: fake_sciter_window_attach_event_handler,
            sciter_window_detach_event_handler: fake_sciter_window_detach_event_handler,
            sciter_value_type,
            sciter_value_string_data,
            #[cfg(windows)]
            sciter_value_init: fake_sciter_value_init,
            #[cfg(windows)]
            sciter_value_clear: fake_sciter_value_clear,
            #[cfg(windows)]
            sciter_value_string_data_set: fake_sciter_value_string_data_set,
            #[cfg(windows)]
            sciter_value_elements_count: fake_sciter_value_elements_count,
            #[cfg(windows)]
            sciter_value_nth_element_value: fake_sciter_value_nth_element_value,
            #[cfg(windows)]
            sciter_value_get_value_of_key: fake_sciter_value_get_value_of_key,
            #[cfg(windows)]
            sciter_update_window: fake_sciter_update_window,
            #[cfg(windows)]
            sciter_exec: fake_sciter_exec,
            #[cfg(windows)]
            sciter_setup_debug_output: fake_sciter_setup_debug_output,
        }
    }

    #[cfg(windows)]
    fn from_bindings(library: WindowsLibrary, bindings: SciterApiBindings) -> Self {
        Self {
            library: Some(library),
            sciter_version: bindings.sciter_version,
            sciter_create_window: bindings.sciter_create_window,
            sciter_load_html: bindings.sciter_load_html,
            sciter_set_option: bindings.sciter_set_option,
            sciter_get_attribute_by_name_cb: bindings.sciter_get_attribute_by_name_cb,
            sciter_get_parent_element: bindings.sciter_get_parent_element,
            sciter_get_root_element: bindings.sciter_get_root_element,
            sciter_eval_element_script: bindings.sciter_eval_element_script,
            sciter_use_element: bindings.sciter_use_element,
            sciter_unuse_element: bindings.sciter_unuse_element,
            sciter_window_attach_event_handler: bindings.sciter_window_attach_event_handler,
            sciter_window_detach_event_handler: bindings.sciter_window_detach_event_handler,
            sciter_value_type: bindings.sciter_value_type,
            sciter_value_string_data: bindings.sciter_value_string_data,
            sciter_value_init: bindings.sciter_value_init,
            sciter_value_clear: bindings.sciter_value_clear,
            sciter_value_string_data_set: bindings.sciter_value_string_data_set,
            sciter_value_elements_count: bindings.sciter_value_elements_count,
            sciter_value_nth_element_value: bindings.sciter_value_nth_element_value,
            sciter_value_get_value_of_key: bindings.sciter_value_get_value_of_key,
            sciter_update_window: bindings.sciter_update_window,
            sciter_exec: bindings.sciter_exec,
            sciter_setup_debug_output: bindings.sciter_setup_debug_output,
        }
    }

    #[cfg(windows)]
    unsafe fn load_bindings_from_api_table(
        library: &WindowsLibrary,
    ) -> Result<SciterApiBindings, SciterRuntimeError> {
        let sciter_api = library.symbol(SCITER_API_EXPORT)?;
        let api = sciter_api();
        if api.is_null() {
            return Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterAPI returned null API pointer".to_string(),
            });
        }
        let api = &*api;

        Ok(SciterApiBindings {
            sciter_version: unsafe { required_api_fn(api.SciterVersion, "SciterVersion")? },
            sciter_create_window: unsafe {
                required_api_fn(api.SciterCreateWindow, "SciterCreateWindow")?
            },
            sciter_load_html: unsafe { required_api_fn(api.SciterLoadHtml, "SciterLoadHtml")? },
            sciter_set_option: unsafe { required_api_fn(api.SciterSetOption, "SciterSetOption")? },
            sciter_get_attribute_by_name_cb: unsafe {
                required_api_fn(api.SciterGetAttributeByNameCB, "SciterGetAttributeByNameCB")?
            },
            sciter_get_parent_element: unsafe {
                required_api_fn(api.SciterGetParentElement, "SciterGetParentElement")?
            },
            sciter_get_root_element: unsafe {
                required_api_fn(api.SciterGetRootElement, "SciterGetRootElement")?
            },
            sciter_eval_element_script: unsafe {
                required_api_fn(api.SciterEvalElementScript, "SciterEvalElementScript")?
            },
            sciter_use_element: unsafe {
                required_api_fn(api.Sciter_UseElement, "Sciter_UseElement")?
            },
            sciter_unuse_element: unsafe {
                required_api_fn(api.Sciter_UnuseElement, "Sciter_UnuseElement")?
            },
            sciter_window_attach_event_handler: unsafe {
                required_api_fn(
                    api.SciterWindowAttachEventHandler,
                    "SciterWindowAttachEventHandler",
                )?
            },
            sciter_window_detach_event_handler: unsafe {
                required_api_fn(
                    api.SciterWindowDetachEventHandler,
                    "SciterWindowDetachEventHandler",
                )?
            },
            sciter_value_type: unsafe { required_api_fn(api.ValueType, "ValueType")? },
            sciter_value_string_data: unsafe {
                required_api_fn(api.ValueStringData, "ValueStringData")?
            },
            sciter_value_init: unsafe { required_api_fn(api.ValueInit, "ValueInit")? },
            sciter_value_clear: unsafe { required_api_fn(api.ValueClear, "ValueClear")? },
            sciter_value_string_data_set: unsafe {
                required_api_fn(api.ValueStringDataSet, "ValueStringDataSet")?
            },
            sciter_value_elements_count: unsafe {
                required_api_fn(api.ValueElementsCount, "ValueElementsCount")?
            },
            sciter_value_nth_element_value: unsafe {
                required_api_fn(api.ValueNthElementValue, "ValueNthElementValue")?
            },
            sciter_value_get_value_of_key: unsafe {
                required_api_fn(api.ValueGetValueOfKey, "ValueGetValueOfKey")?
            },
            sciter_update_window: unsafe {
                required_api_fn(api.SciterUpdateWindow, "SciterUpdateWindow")?
            },
            sciter_exec: unsafe { required_api_fn(api.SciterExec, "SciterExec")? },
            sciter_setup_debug_output: unsafe {
                required_api_fn(api.SciterSetupDebugOutput, "SciterSetupDebugOutput")?
            },
        })
    }
}

#[cfg(windows)]
fn capture_window_placement(window: SciterWindowHandle) -> Option<WindowPlacement> {
    if window.is_null() {
        return None;
    }

    let mut placement = WindowPlacement {
        length: std::mem::size_of::<WindowPlacement>() as u32,
        flags: 0,
        show_cmd: 0,
        min_position: WindowPoint::default(),
        max_position: WindowPoint::default(),
        normal_position: SciterRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
    };

    let captured = unsafe { GetWindowPlacement(window, &mut placement) };
    if captured == 0 {
        None
    } else {
        Some(placement)
    }
}

#[cfg(windows)]
fn restore_window_placement(window: SciterWindowHandle, mut placement: WindowPlacement) {
    if window.is_null() {
        return;
    }

    placement.length = std::mem::size_of::<WindowPlacement>() as u32;
    if placement.show_cmd == SW_SHOWMINIMIZED as u32 {
        placement.show_cmd = SW_RESTORE as u32;
    }

    unsafe {
        let _ = SetWindowPlacement(window, &placement);
    }
}

#[cfg(windows)]
fn capture_normal_window_geometry(window: SciterWindowHandle) {
    if window.is_null() {
        return;
    }

    let Some(placement) = capture_window_placement(window) else {
        return;
    };

    let normal = placement.normal_position;
    let geometry = crate::settings::WindowGeometry {
        left: normal.left,
        top: normal.top,
        right: normal.right,
        bottom: normal.bottom,
    };

    unsafe {
        DL_CAPTURED_GEOMETRY = Some(geometry);
    }
}

#[cfg(windows)]
pub(crate) fn take_captured_window_geometry() -> Option<crate::settings::WindowGeometry> {
    unsafe { std::ptr::replace(std::ptr::addr_of_mut!(DL_CAPTURED_GEOMETRY), None) }
}

#[cfg(not(windows))]
pub(crate) fn take_captured_window_geometry() -> Option<crate::settings::WindowGeometry> {
    None
}

fn viewer_window_creation_flags() -> u32 {
    // The HTML shell owns the only visible top bar, so the host window must not add native title
    // text or window controls above it.
    SW_MAIN | SW_RESIZEABLE
}

fn read_window_cascade_offset() -> (i32, i32) {
    let left = std::env::var("MDLUMA_WINDOW_CASCADE_LEFT")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let top = std::env::var("MDLUMA_WINDOW_CASCADE_TOP")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    (left, top)
}

#[cfg(windows)]
fn is_on_screen_rect(rect: &SciterRect) -> bool {
    let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if vw <= 0 || vh <= 0 {
        return true;
    }
    rect.right > vx && rect.left < vx + vw && rect.bottom > vy && rect.top < vy + vh
}

#[cfg(not(windows))]
fn is_on_screen_rect(_rect: &SciterRect) -> bool {
    true
}

#[cfg(windows)]
fn strip_native_titlebar_style(style: isize) -> isize {
    let remove_bits = (WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX) as isize;
    style & !remove_bits
}

fn should_install_init_script() -> bool {
    !matches!(
        std::env::var("MDLUMA_SCITER_DISABLE_INIT_SCRIPT"),
        Ok(value) if value == "1"
    )
}

#[cfg(windows)]
unsafe extern "C" fn sciter_debug_output_proc(
    _param: *mut core::ffi::c_void,
    _subsystem: u32,
    _severity: u32,
    text: *const u16,
    text_length: u32,
) {
    if text.is_null() {
        crate::debug_log!("[sciter:{_subsystem}:{_severity}] <null>");
        return;
    }

    let text_slice = unsafe { std::slice::from_raw_parts(text, text_length as usize) };
    let message = String::from_utf16_lossy(text_slice);
    let message = message.trim_end_matches(['\r', '\n']);
    if message.is_empty() {
        return;
    }
    crate::debug_log!("[sciter:{_subsystem}:{_severity}] {message}");
}

#[cfg(all(test, windows))]
unsafe extern "C" fn fake_sciter_setup_debug_output(
    _hwnd_or_null: SciterWindowHandle,
    _param: *mut core::ffi::c_void,
    _output: Option<SciterDebugOutputProc>,
) {
}

#[cfg(test)]
unsafe extern "C" fn fake_sciter_window_attach_event_handler(
    _hwnd: SciterWindowHandle,
    _event_proc: SciterElementEventProc,
    _tag: *mut core::ffi::c_void,
    _subscription: u32,
) -> i32 {
    SCDOM_OK
}

#[cfg(test)]
unsafe extern "C" fn fake_sciter_window_detach_event_handler(
    _hwnd: SciterWindowHandle,
    _event_proc: SciterElementEventProc,
    _tag: *mut core::ffi::c_void,
) -> i32 {
    SCDOM_OK
}

#[cfg(test)]
unsafe extern "C" fn fake_sciter_get_attribute_by_name_cb(
    _element: SciterElementHandle,
    _name: *const i8,
    _receiver: SciterLpcwstrReceiver,
    _receiver_param: *mut core::ffi::c_void,
) -> i32 {
    SCDOM_OK
}

#[cfg(test)]
unsafe extern "C" fn fake_sciter_get_parent_element(
    _element: SciterElementHandle,
    _parent: *mut SciterElementHandle,
) -> i32 {
    SCDOM_OK
}

#[cfg(test)]
unsafe extern "C" fn fake_sciter_get_root_element(
    _window: SciterWindowHandle,
    root: *mut SciterElementHandle,
) -> i32 {
    if !root.is_null() {
        unsafe {
            *root = std::ptr::dangling_mut::<core::ffi::c_void>();
        }
    }
    SCDOM_OK
}

#[cfg(test)]
unsafe extern "C" fn fake_sciter_eval_element_script(
    _element: SciterElementHandle,
    _script: *const u16,
    _script_length: u32,
    _retval: *mut SciterValue,
) -> i32 {
    SCDOM_OK
}

#[cfg(test)]
unsafe extern "C" fn fake_sciter_element_ref(_element: SciterElementHandle) -> i32 {
    SCDOM_OK
}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_exec(_cmd: u32, _p1: usize, _p2: usize) -> isize {
    0
}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_update_window(_hwnd: SciterWindowHandle) {}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_value_init(_pval: *mut SciterValue) -> u32 {
    0
}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_value_clear(_pval: *mut SciterValue) -> u32 {
    0
}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_value_string_data_set(
    _pval: *mut SciterValue,
    _chars: *const u16,
    _num_chars: u32,
    _units: u32,
) -> u32 {
    0
}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_value_elements_count(
    _pval: *const SciterValue,
    _pn: *mut i32,
) -> u32 {
    1
}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_value_nth_element_value(
    _pval: *const SciterValue,
    _n: i32,
    _pretval: *mut SciterValue,
) -> u32 {
    1
}

#[cfg(windows)]
#[cfg(test)]
unsafe extern "C" fn fake_sciter_value_get_value_of_key(
    _pval: *const SciterValue,
    _pkey: *const SciterValue,
    _pretval: *mut SciterValue,
) -> u32 {
    1
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct SciterExportSymbol<T> {
    bytes: &'static [u8],
    symbol_name: &'static str,
    _marker: std::marker::PhantomData<T>,
}

#[cfg(windows)]
impl<T> SciterExportSymbol<T> {
    const fn new(bytes: &'static [u8], symbol_name: &'static str) -> Self {
        Self {
            bytes,
            symbol_name,
            _marker: std::marker::PhantomData,
        }
    }
}

#[cfg(windows)]
const SCITER_API_EXPORT: SciterExportSymbol<
    unsafe extern "C" fn() -> *const generated::ISciterAPI,
> = SciterExportSymbol::new(b"SciterAPI\0", "SciterAPI");

#[cfg(windows)]
/// Converts raw Sciter symbol addresses into strongly typed function pointers inside the FFI
/// boundary.
///
/// # Safety
/// Callers must ensure the address is non-null and points to a valid Sciter function whose
/// signature and calling convention exactly match the implementor type.
trait SciterFunctionPointer: Copy {
    unsafe fn from_address(address: *mut core::ffi::c_void) -> Self;
}

#[cfg(windows)]
macro_rules! impl_sciter_function_pointer {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl SciterFunctionPointer for $ty {
                unsafe fn from_address(address: *mut core::ffi::c_void) -> Self {
                    // These implementors are concrete function pointer types for the Sciter C API,
                    // so reinterpreting the symbol address preserves the ABI when the caller has
                    // already verified that the resolved symbol matches the expected signature.
                    unsafe { std::mem::transmute::<*mut core::ffi::c_void, Self>(address) }
                }
            }
        )+
    };
}

#[cfg(windows)]
impl_sciter_function_pointer!(
    unsafe extern "C" fn() -> *const generated::ISciterAPI,
    SciterVersionFn,
    SciterCreateWindowFn,
    SciterLoadHtmlFn,
    SciterSetOptionFn,
    SciterGetAttributeByNameCbFn,
    SciterGetParentElementFn,
    SciterEvalElementScriptFn,
    SciterElementRefFn,
    SciterWindowAttachEventHandlerFn,
    SciterWindowDetachEventHandlerFn,
    SciterValueTypeFn,
    SciterValueStringDataFn,
    SciterValueInitFn,
    SciterValueStringDataSetFn,
    SciterValueElementsCountFn,
    SciterValueNthElementValueFn,
    SciterValueGetValueOfKeyFn,
    SciterUpdateWindowFn,
    SciterExecFn,
    SciterSetupDebugOutputFn,
);

#[cfg(windows)]
unsafe fn required_api_fn<S, T>(
    slot: Option<S>,
    symbol_name: &'static str,
) -> Result<T, SciterRuntimeError>
where
    S: Copy,
    T: SciterFunctionPointer,
{
    let Some(slot) = slot else {
        return Err(SciterRuntimeError::ApiUnavailable {
            message: format!("required Sciter symbol {} is unavailable", symbol_name),
        });
    };
    let address = unsafe { std::mem::transmute_copy::<S, *mut core::ffi::c_void>(&slot) };
    if address.is_null() {
        Err(SciterRuntimeError::ApiUnavailable {
            message: format!("required Sciter symbol {} is unavailable", symbol_name),
        })
    } else {
        Ok(unsafe { T::from_address(address) })
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsLibrary {
    handle: *mut core::ffi::c_void,
}

#[cfg(windows)]
impl WindowsLibrary {
    fn load(path: &Path) -> Result<Self, SciterRuntimeError> {
        let encoded = wide_null(path.as_os_str().to_string_lossy().as_ref());
        let handle = unsafe { LoadLibraryW(encoded.as_ptr()) };

        if handle.is_null() {
            Err(SciterRuntimeError::ApiUnavailable {
                message: format!("could not load {SCITER_DLL_NAME} from {}", path.display()),
            })
        } else {
            Ok(Self { handle })
        }
    }

    unsafe fn symbol<T>(&self, symbol: SciterExportSymbol<T>) -> Result<T, SciterRuntimeError>
    where
        T: SciterFunctionPointer,
    {
        let address = GetProcAddress(self.handle, symbol.bytes.as_ptr() as *const i8);
        if address.is_null() {
            Err(SciterRuntimeError::ApiUnavailable {
                message: format!(
                    "required Sciter symbol {} is unavailable",
                    symbol.symbol_name
                ),
            })
        } else {
            Ok(unsafe { T::from_address(address) })
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsLibrary {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                FreeLibrary(self.handle);
            }
        }
    }
}

#[cfg(windows)]
#[link(name = "Kernel32")]
extern "system" {
    fn LoadLibraryW(path: *const u16) -> *mut core::ffi::c_void;
    fn GetProcAddress(handle: *mut core::ffi::c_void, symbol: *const i8) -> *mut core::ffi::c_void;
    fn FreeLibrary(handle: *mut core::ffi::c_void) -> i32;
}

#[cfg(windows)]
#[link(name = "User32")]
extern "system" {
    fn ShowWindow(window: SciterWindowHandle, command: i32) -> i32;
    fn GetWindowLongPtrW(window: SciterWindowHandle, index: i32) -> isize;
    fn SetWindowLongPtrW(window: SciterWindowHandle, index: i32, value: isize) -> isize;
    fn SetWindowPos(
        window: SciterWindowHandle,
        insert_after: SciterWindowHandle,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    fn CallWindowProcW(
        prev_proc: isize,
        hwnd: SciterWindowHandle,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
    fn PostMessageW(hwnd: SciterWindowHandle, msg: u32, wparam: usize, lparam: isize) -> i32;
    fn GetWindowPlacement(window: SciterWindowHandle, placement: *mut WindowPlacement) -> i32;
    fn SetWindowPlacement(window: SciterWindowHandle, placement: *const WindowPlacement) -> i32;
    fn DragAcceptFiles(hwnd: SciterWindowHandle, accept: i32);
}

#[cfg(windows)]
#[link(name = "Shell32")]
extern "system" {
    fn DragQueryFileW(hdrop: *mut core::ffi::c_void, index: u32, file: *mut u16, cch: u32) -> u32;
    fn DragFinish(hdrop: *mut core::ffi::c_void);
}

#[cfg(windows)]
type WndProcFn = unsafe extern "system" fn(SciterWindowHandle, u32, usize, isize) -> isize;

#[cfg(windows)]
#[derive(Clone)]
struct PendingDeferredLoad {
    load_fn: SciterLoadHtmlFn,
    html: String,
    placement: Option<WindowPlacement>,
}

#[cfg(windows)]
static mut DL_ORIGINAL_PROC: Option<WndProcFn> = None;

#[cfg(windows)]
static mut DL_PENDING_HTML: Option<PendingDeferredLoad> = None;

#[cfg(windows)]
static mut DL_PENDING_PLACEMENT_RESTORE: Option<WindowPlacement> = None;

// SAFETY: This is accessed exclusively from the single-window UI thread via the
// subclassed WndProc and the public take function called at shutdown.
#[cfg(windows)]
static mut DL_CAPTURED_GEOMETRY: Option<crate::settings::WindowGeometry> = None;

#[cfg(windows)]
static mut DL_POSITION_LOCKED: bool = false;

#[cfg(windows)]
static mut DL_GET_ROOT_ELEMENT_FN: Option<SciterGetRootElementFn> = None;

#[cfg(windows)]
static mut DL_EVAL_ELEMENT_SCRIPT_FN: Option<SciterEvalElementScriptFn> = None;

// SAFETY: These `static mut` variables are accessed exclusively from the single-window
// thread (the UI message loop) via the subclassed WndProc. They are never read or written
// from any other thread, so no data race is possible. If multi-window or multi-threaded
// access is ever introduced, these must be migrated to thread-local storage or a Mutex.

#[cfg(windows)]
#[repr(C)]
struct WindowPos {
    hwnd: SciterWindowHandle,
    insert_after: SciterWindowHandle,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    flags: u32,
}

#[cfg(windows)]
type NativeDropDispatchFn = unsafe extern "system" fn(*mut core::ffi::c_void);

// SAFETY: These are accessed exclusively from the single-window UI thread (see comment above).
#[cfg(windows)]
static mut DL_DROP_DISPATCH_FN: Option<NativeDropDispatchFn> = None;

#[cfg(windows)]
static mut DL_DROP_DISPATCH_CTX: *mut core::ffi::c_void = std::ptr::null_mut();

#[cfg(windows)]
pub(crate) fn set_native_drop_dispatch(
    ctx: *mut core::ffi::c_void,
    dispatch: NativeDropDispatchFn,
) {
    unsafe {
        DL_DROP_DISPATCH_FN = Some(dispatch);
        DL_DROP_DISPATCH_CTX = ctx;
    }
}

#[cfg(windows)]
static NATIVE_DROP_QUEUE: OnceLock<Mutex<Vec<Vec<PathBuf>>>> = OnceLock::new();

#[cfg(windows)]
unsafe extern "system" fn deferred_load_wnd_proc(
    hwnd: SciterWindowHandle,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    // Right-clicks on window-caption are handled as non-client messages on Windows.
    // Route this path into JS so recent-files popup behavior stays available even
    // when Sciter does not dispatch DOM contextmenu events for caption elements.
    if msg == WM_NCRBUTTONUP && wparam == HTCAPTION {
        if show_recent_files_popup_from_native_caption(hwnd) {
            return 0;
        }
    }

    if msg == WM_DROPFILES {
        let hdrop = wparam as *mut core::ffi::c_void;
        let dropped_paths = collect_dropped_file_paths(hdrop);
        if !dropped_paths.is_empty() {
            let queue = NATIVE_DROP_QUEUE.get_or_init(|| Mutex::new(Vec::new()));
            if let Ok(mut pending) = queue.lock() {
                pending.push(dropped_paths);
            }
        }
        unsafe {
            DragFinish(hdrop);
        }
        let dispatch_fn = std::ptr::read(std::ptr::addr_of!(DL_DROP_DISPATCH_FN));
        let dispatch_ctx = std::ptr::read(std::ptr::addr_of!(DL_DROP_DISPATCH_CTX));
        if let Some(dispatch) = dispatch_fn {
            if !dispatch_ctx.is_null() {
                unsafe { dispatch(dispatch_ctx) };
            }
        }
        return 0;
    }

    if msg == WM_WINDOWPOSCHANGING {
        if std::ptr::read(std::ptr::addr_of!(DL_POSITION_LOCKED)) {
            let pos = &mut *(lparam as *mut WindowPos);
            pos.flags |= SWP_NOMOVE | SWP_NOSIZE;
        }
    }

    if msg == WM_CLOSE {
        capture_normal_window_geometry(hwnd);
    }

    if msg == WM_DL_DEFERRED_LOAD {
        let pending = std::ptr::replace(std::ptr::addr_of_mut!(DL_PENDING_HTML), None);
        if let Some(pending) = pending {
            std::ptr::write(std::ptr::addr_of_mut!(DL_POSITION_LOCKED), true);
            let _ = (pending.load_fn)(
                hwnd,
                pending.html.as_ptr(),
                pending.html.len() as u32,
                std::ptr::null(),
            );
            std::ptr::write(std::ptr::addr_of_mut!(DL_POSITION_LOCKED), false);
            if let Some(placement) = pending.placement {
                restore_window_placement(hwnd, placement);
                std::ptr::write(
                    std::ptr::addr_of_mut!(DL_PENDING_PLACEMENT_RESTORE),
                    Some(placement),
                );
            }
            DragAcceptFiles(hwnd, 0);
            DragAcceptFiles(hwnd, 1);
        }
        return 0;
    }
    if let Some(original) = std::ptr::read(std::ptr::addr_of!(DL_ORIGINAL_PROC)) {
        CallWindowProcW(original as isize, hwnd, msg, wparam, lparam)
    } else {
        0
    }
}

#[cfg(windows)]
fn show_recent_files_popup_from_native_caption(hwnd: SciterWindowHandle) -> bool {
    let get_root = unsafe { std::ptr::read(std::ptr::addr_of!(DL_GET_ROOT_ELEMENT_FN)) };
    let eval_script = unsafe { std::ptr::read(std::ptr::addr_of!(DL_EVAL_ELEMENT_SCRIPT_FN)) };
    let (Some(get_root), Some(eval_script)) = (get_root, eval_script) else {
        return false;
    };

    let mut root = std::ptr::null_mut();
    let root_status = unsafe { get_root(hwnd, &mut root) };
    if root_status != SCDOM_OK || root.is_null() {
        return false;
    }

    let script: Vec<u16> = "globalThis.__mdlumaShowRecentFilesFromNativeCaption && globalThis.__mdlumaShowRecentFilesFromNativeCaption();"
        .encode_utf16()
        .collect();
    let status = unsafe {
        eval_script(
            root,
            script.as_ptr(),
            script.len() as u32,
            std::ptr::null_mut(),
        )
    };

    status == SCDOM_OK
}

#[cfg(windows)]
fn collect_dropped_file_paths(hdrop: *mut core::ffi::c_void) -> Vec<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    if hdrop.is_null() {
        return Vec::new();
    }

    let file_count = unsafe { DragQueryFileW(hdrop, u32::MAX, std::ptr::null_mut(), 0) };
    let mut paths = Vec::with_capacity(file_count as usize);

    for index in 0..file_count {
        let len = unsafe { DragQueryFileW(hdrop, index, std::ptr::null_mut(), 0) };
        if len == 0 {
            continue;
        }
        let mut buffer = vec![0u16; (len + 1) as usize];
        let written = unsafe { DragQueryFileW(hdrop, index, buffer.as_mut_ptr(), len + 1) };
        if written == 0 {
            continue;
        }
        let os = OsString::from_wide(&buffer[..written as usize]);
        paths.push(PathBuf::from(os));
    }

    paths
}

#[cfg(windows)]
pub(crate) fn take_pending_native_drops() -> Vec<Vec<PathBuf>> {
    let Some(queue) = NATIVE_DROP_QUEUE.get() else {
        return Vec::new();
    };
    let Ok(mut pending) = queue.lock() else {
        return Vec::new();
    };
    std::mem::take(&mut *pending)
}

#[cfg(not(windows))]
pub(crate) fn take_pending_native_drops() -> Vec<Vec<std::path::PathBuf>> {
    Vec::new()
}

#[cfg(not(windows))]
pub(crate) fn set_native_drop_dispatch(
    _ctx: *mut core::ffi::c_void,
    _dispatch: unsafe extern "system" fn(*mut core::ffi::c_void),
) {
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn store_utf16_string(
    value: *const u16,
    value_length: u32,
    param: *mut core::ffi::c_void,
) {
    if param.is_null() {
        return;
    }

    let storage = unsafe { &mut *(param as *mut String) };
    if value.is_null() {
        storage.clear();
        return;
    }

    let slice = unsafe { std::slice::from_raw_parts(value, value_length as usize) };
    *storage = String::from_utf16_lossy(slice);
}

#[cfg(windows)]
fn sciter_file_uri_to_path(s: &str) -> PathBuf {
    if let Some(stripped) = s.strip_prefix("file:///") {
        let path = stripped.replace('/', "\\");
        PathBuf::from(path)
    } else if let Some(stripped) = s.strip_prefix("file://") {
        PathBuf::from(stripped.replace('/', "\\"))
    } else {
        PathBuf::from(s)
    }
}

pub fn sciter_value_to_string(api: &SciterApi, value: &SciterValue) -> Option<String> {
    const SCITER_T_STRING: u32 = 5;
    let (value_type, _units) = api.sciter_value_type(value)?;
    if (value_type & 0x0F) != SCITER_T_STRING {
        return None;
    }
    api.sciter_value_string_data(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn sciter_api_symbol_types_stay_inside_ffi_boundary() {
        assert_eq!(
            std::mem::size_of::<Option<SciterCallback>>(),
            std::mem::size_of::<usize>()
        );
        assert_eq!(
            std::mem::size_of::<SciterWindowHandle>(),
            std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn sciter_value_to_string_returns_none_for_non_string_type() {
        let api = SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window_success,
            fake_sciter_load_html_success,
            fake_sciter_set_option_success,
            fake_sciter_value_type_returns_zero,
            fake_sciter_value_string_data_never_called,
        );
        let value = SciterValue::default();
        assert_eq!(sciter_value_to_string(&api, &value), None);
    }

    #[test]
    fn sciter_value_to_string_delegates_to_api_for_string_values() {
        let api = SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window_success,
            fake_sciter_load_html_success,
            fake_sciter_set_option_success,
            fake_sciter_value_type_returns_string,
            fake_sciter_value_string_data_returns_hello,
        );
        let value = SciterValue::default();
        assert_eq!(
            sciter_value_to_string(&api, &value),
            Some("Hello".to_string())
        );
    }

    #[test]
    fn create_window_reports_typed_error_when_runtime_returns_null() {
        let api = SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window_null,
            fake_sciter_load_html_success,
            fake_sciter_set_option_success,
            fake_sciter_value_type_returns_zero,
            fake_sciter_value_string_data_never_called,
        );

        let error = api
            .create_window(None)
            .expect_err("null window handle should fail");
        assert!(matches!(error, SciterRuntimeError::ApiUnavailable { .. }));
    }

    #[test]
    fn create_window_uses_visible_main_window_flags_and_frame() {
        LAST_CREATE_WINDOW_FLAGS.store(0, Ordering::SeqCst);
        LAST_CREATE_WINDOW_FRAME_NON_NULL.store(false, Ordering::SeqCst);
        LAST_SET_OPTION_ID.store(0, Ordering::SeqCst);
        LAST_SET_OPTION_VALUE_NON_NULL.store(false, Ordering::SeqCst);
        let api = SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window_records_args,
            fake_sciter_load_html_success,
            fake_sciter_set_option_records_args,
            fake_sciter_value_type_returns_zero,
            fake_sciter_value_string_data_never_called,
        );

        let _window = api.create_window(None).expect("create fake window");

        assert_eq!(
            LAST_CREATE_WINDOW_FLAGS.load(Ordering::SeqCst),
            viewer_window_creation_flags()
        );
        assert!(LAST_CREATE_WINDOW_FRAME_NON_NULL.load(Ordering::SeqCst));
        assert_eq!(
            LAST_SET_OPTION_ID.load(Ordering::SeqCst),
            SCITER_SET_DEBUG_MODE
        );
        assert!(LAST_SET_OPTION_VALUE_NON_NULL.load(Ordering::SeqCst));
    }

    #[test]
    fn create_window_strips_native_caption_styles_and_refreshes_frame() {
        let original_style =
            (WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX) as isize | 0x00040000isize;

        let stripped = strip_native_titlebar_style(original_style);

        assert_eq!(stripped & (WS_CAPTION as isize), 0);
        assert_eq!(stripped & (WS_SYSMENU as isize), 0);
        assert_eq!(stripped & (WS_MINIMIZEBOX as isize), 0);
        assert_eq!(stripped & (WS_MAXIMIZEBOX as isize), 0);
        assert_ne!(stripped & 0x00040000isize, 0);
    }

    #[test]
    fn viewer_window_creation_flags_leave_native_title_row_and_controls_disabled() {
        let flags = viewer_window_creation_flags();

        assert_eq!(flags, SW_MAIN | SW_RESIZEABLE);
        assert_eq!(flags & SW_TITLEBAR, 0);
        assert_eq!(flags & SW_CONTROLS, 0);
    }

    #[test]
    fn load_html_reports_typed_error_when_runtime_rejects_html() {
        let api = SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window_success,
            fake_sciter_load_html_failure,
            fake_sciter_set_option_success,
            fake_sciter_value_type_returns_zero,
            fake_sciter_value_string_data_never_called,
        );

        let window = api.create_window(None).expect("create fake window");
        let error = api
            .load_html(window, "<html></html>")
            .expect_err("load html failure should be typed");
        assert!(matches!(error, SciterRuntimeError::ApiUnavailable { .. }));
    }

    #[test]
    fn load_html_uses_null_base_url_for_local_only_documents() {
        BASE_URL_WAS_NULL.store(false, Ordering::SeqCst);
        let api = SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window_success,
            fake_sciter_load_html_records_null_base_url,
            fake_sciter_set_option_success,
            fake_sciter_value_type_returns_zero,
            fake_sciter_value_string_data_never_called,
        );

        let window = api.create_window(None).expect("create fake window");
        api.load_html(window, "<html></html>")
            .expect("load html with local-only base url");

        assert!(BASE_URL_WAS_NULL.load(Ordering::SeqCst));
    }

    #[test]
    fn create_window_reports_typed_error_when_init_script_configuration_fails() {
        let api = SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window_success,
            fake_sciter_load_html_success,
            fake_sciter_set_option_fail_init_script_only,
            fake_sciter_value_type_returns_zero,
            fake_sciter_value_string_data_never_called,
        );

        let error = api
            .create_window(None)
            .expect_err("init script failure should abort window creation");

        assert!(matches!(error, SciterRuntimeError::ApiUnavailable { .. }));
        assert!(error
            .operator_diagnostic()
            .contains("extended-frame init script"));
    }

    #[test]
    fn non_windows_loader_reports_typed_runtime_error() {
        #[cfg(not(windows))]
        {
            let error = SciterApi::load(Path::new("sciter.dll"))
                .expect_err("non-Windows build should not load Sciter API");

            assert!(matches!(error, SciterRuntimeError::ApiUnavailable { .. }));
        }

        #[cfg(windows)]
        {
            let _ = Path::new("sciter.dll");
        }
    }

    unsafe extern "C" fn fake_sciter_version(n: u32) -> u32 {
        match n {
            0 => 6,
            1 => 0,
            _ => 0,
        }
    }

    unsafe extern "C" fn fake_sciter_create_window_success(
        _creation_flags: u32,
        _frame: *const core::ffi::c_void,
        _delegate: Option<SciterCallback>,
        _delegate_param: *mut core::ffi::c_void,
        _parent: SciterWindowHandle,
    ) -> SciterWindowHandle {
        std::ptr::dangling_mut::<core::ffi::c_void>()
    }

    static LAST_CREATE_WINDOW_FLAGS: std::sync::atomic::AtomicU32 =
        std::sync::atomic::AtomicU32::new(0);
    static LAST_CREATE_WINDOW_FRAME_NON_NULL: AtomicBool = AtomicBool::new(false);
    static LAST_SET_OPTION_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    static LAST_SET_OPTION_VALUE_NON_NULL: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn fake_sciter_create_window_records_args(
        creation_flags: u32,
        frame: *const core::ffi::c_void,
        _delegate: Option<SciterCallback>,
        _delegate_param: *mut core::ffi::c_void,
        _parent: SciterWindowHandle,
    ) -> SciterWindowHandle {
        LAST_CREATE_WINDOW_FLAGS.store(creation_flags, Ordering::SeqCst);
        LAST_CREATE_WINDOW_FRAME_NON_NULL.store(!frame.is_null(), Ordering::SeqCst);
        std::ptr::dangling_mut::<core::ffi::c_void>()
    }

    unsafe extern "C" fn fake_sciter_create_window_null(
        _creation_flags: u32,
        _frame: *const core::ffi::c_void,
        _delegate: Option<SciterCallback>,
        _delegate_param: *mut core::ffi::c_void,
        _parent: SciterWindowHandle,
    ) -> SciterWindowHandle {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn fake_sciter_load_html_success(
        _hwnd: SciterWindowHandle,
        _html: *const u8,
        _html_length: u32,
        _base_url: *const u16,
    ) -> i32 {
        1
    }

    static BASE_URL_WAS_NULL: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn fake_sciter_load_html_records_null_base_url(
        _hwnd: SciterWindowHandle,
        _html: *const u8,
        _html_length: u32,
        base_url: *const u16,
    ) -> i32 {
        BASE_URL_WAS_NULL.store(base_url.is_null(), Ordering::SeqCst);
        1
    }

    unsafe extern "C" fn fake_sciter_load_html_failure(
        _hwnd: SciterWindowHandle,
        _html: *const u8,
        _html_length: u32,
        _base_url: *const u16,
    ) -> i32 {
        0
    }

    unsafe extern "C" fn fake_sciter_set_option_success(
        _hwnd: SciterWindowHandle,
        _option: u32,
        _value: usize,
    ) -> i32 {
        1
    }

    unsafe extern "C" fn fake_sciter_set_option_records_args(
        _hwnd: SciterWindowHandle,
        option: u32,
        value: usize,
    ) -> i32 {
        LAST_SET_OPTION_ID.store(option, Ordering::SeqCst);
        LAST_SET_OPTION_VALUE_NON_NULL.store(value != 0, Ordering::SeqCst);
        1
    }

    unsafe extern "C" fn fake_sciter_set_option_fail_init_script_only(
        _hwnd: SciterWindowHandle,
        option: u32,
        _value: usize,
    ) -> i32 {
        if option == SCITER_SET_INIT_SCRIPT {
            0
        } else {
            1
        }
    }

    unsafe extern "C" fn fake_sciter_value_type_returns_zero(
        _pval: *const SciterValue,
        _p_type: *mut u32,
        _p_units: *mut u32,
    ) -> u32 {
        1
    }

    unsafe extern "C" fn fake_sciter_value_type_returns_string(
        _pval: *const SciterValue,
        p_type: *mut u32,
        _p_units: *mut u32,
    ) -> u32 {
        unsafe {
            *p_type = 5;
        }
        0
    }

    unsafe extern "C" fn fake_sciter_value_string_data_never_called(
        _pval: *const SciterValue,
        _p_chars: *mut *const u16,
        _p_num_chars: *mut u32,
    ) -> u32 {
        1
    }

    unsafe extern "C" fn fake_sciter_value_string_data_returns_hello(
        _pval: *const SciterValue,
        p_chars: *mut *const u16,
        p_num_chars: *mut u32,
    ) -> u32 {
        static HELLO: &[u16] = &['H' as u16, 'e' as u16, 'l' as u16, 'l' as u16, 'o' as u16];
        unsafe {
            *p_chars = HELLO.as_ptr();
            *p_num_chars = 5;
        }
        0
    }
}
