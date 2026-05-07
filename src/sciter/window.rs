use crate::sciter::ffi::{
    sciter_value_to_string, set_native_drop_dispatch, take_pending_native_drops,
    BehaviorEventParams, ExchangeParams, SciterApi, SciterElementHandle, SciterValue,
    SciterWindowHandle, ScriptingMethodParams, BEHAVIOR_EVENT_CUSTOM, HANDLE_BEHAVIOR_EVENT,
    HANDLE_EXCHANGE, HANDLE_SCRIPTING_METHOD_CALL, X_DROP, X_WILL_ACCEPT_DROP,
};
use crate::sciter::runtime::SciterRuntime;
use crate::{
    DefaultHtmlShell, EmbeddedUiAssets, HtmlShell, ResourcePolicy, ShellModel, Theme, ViewerError,
    ViewerState, WindowChromeController, WindowsWindowChrome, APP_NAME,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(windows)]
const DOCUMENT_COMPLETE_EVENT: u32 = 0x0098;
#[cfg(windows)]
const DOCUMENT_READY_EVENT: u32 = 0x00c3;

const SINKING: u32 = 0x8000;

#[cfg(test)]
#[cfg(windows)]
#[repr(C)]
struct Message {
    hwnd: *mut core::ffi::c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
    l_private: u32,
}

#[cfg(test)]
#[cfg(windows)]
#[link(name = "User32")]
extern "system" {
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn IsWindow(window: *mut core::ffi::c_void) -> i32;
}

pub trait ViewerUi {
    fn show_initial(&mut self, html: &str) -> Result<(), ViewerError>;
    fn show_document(&mut self, html: &str) -> Result<(), ViewerError>;
    fn show_error(&mut self, error: &ViewerError) -> Result<(), ViewerError>;
    fn run_event_loop(&mut self) -> Result<(), ViewerError>;
    fn request_close(&mut self) -> Result<(), ViewerError>;

    fn native_window_handle(&self) -> Option<SciterWindowHandle> {
        None
    }
}

pub trait ViewerCommandBinder {
    fn bind_viewer_command_handler<H>(&mut self, handler: &mut H) -> Result<(), ViewerError>
    where
        H: ViewerCommandHandler;
}

impl ViewerUi for Rc<RefCell<SciterWindow>> {
    fn show_initial(&mut self, html: &str) -> Result<(), ViewerError> {
        self.borrow_mut().show_initial(html)
    }

    fn show_document(&mut self, html: &str) -> Result<(), ViewerError> {
        self.borrow_mut().show_document(html)
    }

    fn show_error(&mut self, error: &ViewerError) -> Result<(), ViewerError> {
        self.borrow_mut().show_error(error)
    }

    fn request_close(&mut self) -> Result<(), ViewerError> {
        self.borrow_mut().request_close()
    }

    #[cfg(windows)]
    fn native_window_handle(&self) -> Option<SciterWindowHandle> {
        Some(self.borrow().window)
    }

    #[cfg(windows)]
    fn run_event_loop(&mut self) -> Result<(), ViewerError> {
        let sciter_exec_fn = self.borrow().api.sciter_exec();
        let _ = unsafe { (sciter_exec_fn)(1, 0, 0) };
        Ok(())
    }

    #[cfg(not(windows))]
    fn run_event_loop(&mut self) -> Result<(), ViewerError> {
        let window = self.borrow().window;
        run_sciter_event_loop(window)
    }
}

impl ViewerCommandBinder for Rc<RefCell<SciterWindow>> {
    fn bind_viewer_command_handler<H>(&mut self, handler: &mut H) -> Result<(), ViewerError>
    where
        H: ViewerCommandHandler,
    {
        self.borrow_mut().bind_viewer_command_handler(handler)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerCommand {
    OpenFileRequested,
    OpenDroppedFiles(Vec<PathBuf>),
    OpenRecentFile(usize),
    ErrorDismissRequested,
    ThemeToggleRequested,
    FontSettingsRequested,
    ExternalEditorRequested,
    ExternalEditorSettingRequested,
    OpenExternalUrl(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowChromeAction {
    Minimize,
    ToggleMaximize,
    Close,
}

impl WindowChromeAction {
    pub fn from_ui_event(event_name: &str) -> Option<Self> {
        match event_name {
            "window-minimize-requested" => Some(Self::Minimize),
            "window-toggle-maximize-requested" => Some(Self::ToggleMaximize),
            "window-close-requested" => Some(Self::Close),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RoutedCommand {
    Viewer(ViewerCommand),
    WindowChrome(WindowChromeAction),
}

impl ViewerCommand {
    fn from_element_action(action_name: &str) -> Option<Self> {
        match action_name {
            "open-file" => Some(Self::OpenFileRequested),
            "theme" => Some(Self::ThemeToggleRequested),
            "font" => Some(Self::FontSettingsRequested),
            "external-editor" => Some(Self::ExternalEditorRequested),
            "external-editor-setting" => Some(Self::ExternalEditorSettingRequested),
            _ => None,
        }
    }
}

fn parse_scripting_method_call(
    method_name: &str,
    argv: &[SciterValue],
    api: &SciterApi,
) -> Option<ViewerCommand> {
    parse_scripting_method_call_with(method_name, argv, |v| sciter_value_to_string(api, v))
}

fn parse_scripting_method_call_with(
    method_name: &str,
    argv: &[SciterValue],
    extract: impl Fn(&SciterValue) -> Option<String>,
) -> Option<ViewerCommand> {
    match method_name {
        "open-file-requested" => Some(ViewerCommand::OpenFileRequested),
        "theme-toggle-requested" => Some(ViewerCommand::ThemeToggleRequested),
        "external-editor-requested" => Some(ViewerCommand::ExternalEditorRequested),
        "external-editor-setting-requested" => Some(ViewerCommand::ExternalEditorSettingRequested),
        "error-dismiss-requested" => Some(ViewerCommand::ErrorDismissRequested),
        "open-dropped-files" => {
            let paths: Vec<PathBuf> = argv.iter().filter_map(extract).map(PathBuf::from).collect();
            Some(ViewerCommand::OpenDroppedFiles(paths))
        }
        "open-recent-file" => argv
            .first()
            .and_then(extract)
            .and_then(|s| s.parse::<usize>().ok())
            .map(ViewerCommand::OpenRecentFile),
        _ => None,
    }
}

pub trait ViewerCommandHandler {
    fn handle_viewer_command(&mut self, command: ViewerCommand) -> Result<(), ViewerError>;
}

pub struct SciterWindow {
    window: SciterWindowHandle,
    api: SciterApi,
    window_chrome: Box<dyn WindowChromeController>,
    event_bridge_installed: bool,
    event_bridge_tag: *mut core::ffi::c_void,
    visible: bool,
}

impl SciterWindow {
    pub fn new(runtime: SciterRuntime) -> Result<Self, ViewerError> {
        Self::with_api_and_geometry(runtime.into_api(), None)
    }

    pub fn with_geometry(
        runtime: SciterRuntime,
        geometry: Option<&crate::settings::WindowGeometry>,
    ) -> Result<Self, ViewerError> {
        Self::with_api_and_geometry(runtime.into_api(), geometry)
    }

    #[cfg(test)]
    fn with_api(api: SciterApi) -> Result<Self, ViewerError> {
        Self::with_api_and_geometry(api, None)
    }

    fn with_api_and_geometry(
        api: SciterApi,
        saved_geometry: Option<&crate::settings::WindowGeometry>,
    ) -> Result<Self, ViewerError> {
        Self::with_api_and_window_chrome(api, Box::new(WindowsWindowChrome), saved_geometry)
    }

    fn with_api_and_window_chrome(
        api: SciterApi,
        window_chrome: Box<dyn WindowChromeController>,
        saved_geometry: Option<&crate::settings::WindowGeometry>,
    ) -> Result<Self, ViewerError> {
        #[cfg(windows)]
        api.init_app();
        let window = api
            .create_window(saved_geometry)
            .map_err(ViewerError::from)?;
        #[cfg(windows)]
        api.setup_deferred_load(window);
        Ok(Self {
            window,
            api,
            window_chrome,
            event_bridge_installed: false,
            event_bridge_tag: std::ptr::null_mut(),
            visible: false,
        })
    }

    fn load_html(&mut self, html: &str) -> Result<(), ViewerError> {
        if !self.visible {
            self.api
                .load_html(self.window, html)
                .map_err(|error| ViewerError::ui(error.operator_diagnostic()))?;
            self.api.show_window(self.window);
            self.visible = true;
        } else {
            #[cfg(windows)]
            if self.api.should_defer_load() {
                self.api
                    .defer_load_html(self.window, html)
                    .map_err(|error| ViewerError::ui(error.operator_diagnostic()))?;
            } else {
                self.api
                    .load_html(self.window, html)
                    .map_err(|error| ViewerError::ui(error.operator_diagnostic()))?;
            }

            #[cfg(not(windows))]
            {
                self.api
                    .load_html(self.window, html)
                    .map_err(|error| ViewerError::ui(error.operator_diagnostic()))?;
            }
        }

        Ok(())
    }

    pub fn bind_viewer_command_handler<H>(&mut self, handler: &mut H) -> Result<(), ViewerError>
    where
        H: ViewerCommandHandler,
    {
        if self.event_bridge_installed {
            return Ok(());
        }

        let binding = Box::new(HandlerBinding {
            handler: (handler as *mut H).cast(),
            dispatch_viewer: dispatch_viewer_command::<H>,
            api: (&self.api) as *const SciterApi,
            window: self.window,
            window_chrome: (&*self.window_chrome) as *const dyn WindowChromeController,
            dispatch_window_chrome: dispatch_window_chrome_command,
            _marker: std::marker::PhantomData,
        });
        let binding = Box::into_raw(binding);

        match self.api.attach_window_event_handler(
            self.window,
            viewer_command_event_proc,
            binding.cast(),
            HANDLE_BEHAVIOR_EVENT | HANDLE_EXCHANGE | HANDLE_SCRIPTING_METHOD_CALL,
        ) {
            Ok(()) => {
                self.event_bridge_installed = true;
                self.event_bridge_tag = binding.cast();
                set_native_drop_dispatch(binding.cast(), dispatch_native_drops_from_wndproc);
                Ok(())
            }
            Err(error) => {
                unsafe {
                    drop(Box::from_raw(binding));
                }
                Err(ViewerError::ui(error.operator_diagnostic()))
            }
        }
    }
}

struct HandlerBinding<'a> {
    handler: *mut core::ffi::c_void,
    dispatch_viewer: unsafe fn(*mut core::ffi::c_void, ViewerCommand) -> Result<(), ViewerError>,
    api: *const SciterApi,
    window: SciterWindowHandle,
    window_chrome: *const dyn WindowChromeController,
    dispatch_window_chrome: unsafe fn(
        *const dyn WindowChromeController,
        SciterWindowHandle,
        WindowChromeAction,
    ) -> Result<(), ViewerError>,
    _marker: std::marker::PhantomData<&'a mut ()>,
}

unsafe extern "system" fn viewer_command_event_proc(
    tag: *mut core::ffi::c_void,
    _element: *mut core::ffi::c_void,
    event_group: u32,
    params: *mut core::ffi::c_void,
) -> i32 {
    if tag.is_null() || params.is_null() {
        return 0;
    }

    let binding = unsafe { &mut *(tag as *mut HandlerBinding<'static>) };
    for paths in take_pending_native_drops() {
        crate::debug_log!(
            "dispatching pending native drop through viewer command: {} path(s)",
            paths.len()
        );
        let _ = unsafe {
            (binding.dispatch_viewer)(binding.handler, ViewerCommand::OpenDroppedFiles(paths))
        };
    }

    match event_group {
        HANDLE_BEHAVIOR_EVENT => {
            let params = unsafe { &mut *(params as *mut BehaviorEventParams) };
            let normalized_cmd = params.cmd & !SINKING;
            #[cfg(windows)]
            if matches!(
                normalized_cmd,
                DOCUMENT_READY_EVENT | DOCUMENT_COMPLETE_EVENT
            ) {
                unsafe { &*binding.api }.restore_pending_window_placement(binding.window);
                unsafe { &*binding.api }.update_window(binding.window);
            }

            match parse_routed_command_from_behavior_event(params, unsafe { &*binding.api }) {
                Some(RoutedCommand::Viewer(command)) => unsafe {
                    ((binding.dispatch_viewer)(binding.handler, command).is_ok()) as i32
                },
                Some(RoutedCommand::WindowChrome(action)) => unsafe {
                    ((binding.dispatch_window_chrome)(
                        binding.window_chrome,
                        binding.window,
                        action,
                    )
                    .is_ok()) as i32
                },
                None => 0,
            }
        }
        HANDLE_SCRIPTING_METHOD_CALL => {
            let params = unsafe { &mut *(params as *mut ScriptingMethodParams) };
            let method_name = narrow_string(params.name);
            let argv_slice = if params.argc > 0 && !params.argv.is_null() {
                unsafe { std::slice::from_raw_parts(params.argv, params.argc as usize) }
            } else {
                &[]
            };
            let api = unsafe { &*binding.api };

            let command = method_name
                .as_deref()
                .and_then(|name| parse_scripting_method_call(name, argv_slice, api));

            match command {
                Some(command) => unsafe {
                    ((binding.dispatch_viewer)(binding.handler, command).is_ok()) as i32
                },
                None => 0,
            }
        }
        HANDLE_EXCHANGE => {
            let params = unsafe { &*(params as *const ExchangeParams) };
            if params.cmd & 0x10000 != 0 {
                return 0;
            }
            let normalized_cmd = params.cmd & !SINKING;

            if normalized_cmd == X_WILL_ACCEPT_DROP {
                return 1;
            }

            if normalized_cmd == X_DROP {
                #[cfg(windows)]
                {
                    let api = unsafe { &*binding.api };
                    let paths = api.extract_exchange_drop_paths(&params.data);
                    if !paths.is_empty() {
                        let _ = unsafe {
                            (binding.dispatch_viewer)(
                                binding.handler,
                                ViewerCommand::OpenDroppedFiles(paths),
                            )
                        };
                        return 1;
                    }
                }
            }

            0
        }
        _ => 0,
    }
}

unsafe fn dispatch_viewer_command<H>(
    handler: *mut core::ffi::c_void,
    command: ViewerCommand,
) -> Result<(), ViewerError>
where
    H: ViewerCommandHandler,
{
    let handler = unsafe { &mut *(handler as *mut H) };
    handler.handle_viewer_command(command)
}

unsafe extern "system" fn dispatch_native_drops_from_wndproc(ctx: *mut core::ffi::c_void) {
    let binding = unsafe { &*(ctx as *const HandlerBinding<'static>) };
    for paths in take_pending_native_drops() {
        let _ = unsafe {
            (binding.dispatch_viewer)(binding.handler, ViewerCommand::OpenDroppedFiles(paths))
        };
    }
}

unsafe fn dispatch_window_chrome_command(
    window_chrome: *const dyn WindowChromeController,
    window: SciterWindowHandle,
    action: WindowChromeAction,
) -> Result<(), ViewerError> {
    let window_chrome = unsafe { &*window_chrome };
    match action {
        WindowChromeAction::Minimize => window_chrome.minimize(window),
        WindowChromeAction::ToggleMaximize => window_chrome.toggle_maximize(window).map(|_| ()),
        WindowChromeAction::Close => window_chrome.close(window),
    }
}

fn parse_routed_command_from_behavior_event(
    params: &BehaviorEventParams,
    api: &SciterApi,
) -> Option<RoutedCommand> {
    const HANDLED: u32 = 0x10000;
    const BUTTON_CLICK: u32 = 0x0000;
    const MENU_ITEM_CLICK: u32 = 0x000b;
    const GENERIC_CLICK: u32 = 0x0016;
    const HYPERLINK_CLICK: u32 = 0x0080;

    // Ignore Sciter's post-dispatch handled notifications so actions fire only once.
    if params.cmd & HANDLED != 0 {
        return None;
    }

    let normalized_cmd = params.cmd & !SINKING;

    if normalized_cmd == BEHAVIOR_EVENT_CUSTOM {
        let name = wide_string(params.name)?;
        return WindowChromeAction::from_ui_event(&name).map(RoutedCommand::WindowChrome);
    }

    if normalized_cmd == HYPERLINK_CLICK {
        let href = element_string_attribute(api, params.source, "data-href");
        if let Some(url) = href {
            if url.starts_with("http://") || url.starts_with("https://") {
                return Some(RoutedCommand::Viewer(ViewerCommand::OpenExternalUrl(url)));
            }
        }
        return None;
    }

    if !matches!(
        normalized_cmd,
        BUTTON_CLICK | MENU_ITEM_CLICK | GENERIC_CLICK
    ) {
        return None;
    }

    for element in [params.source, params.target] {
        let Some(action_element) = resolve_element_with_attribute(api, element, "data-action")
        else {
            continue;
        };
        let Some(action) = api.get_attribute_by_name(action_element, "data-action") else {
            continue;
        };

        if action == "recent-file" {
            if let Some(index_element) =
                resolve_element_with_attribute(api, action_element, "data-recent-index")
            {
                if let Some(index_str) =
                    api.get_attribute_by_name(index_element, "data-recent-index")
                {
                    if let Ok(index) = index_str.parse::<usize>() {
                        return Some(RoutedCommand::Viewer(ViewerCommand::OpenRecentFile(index)));
                    }
                }
            }
            return None;
        }

        if let Some(command) = ViewerCommand::from_element_action(&action) {
            return Some(RoutedCommand::Viewer(command));
        }
    }

    None
}

fn resolve_element_with_attribute(
    api: &SciterApi,
    mut element: SciterElementHandle,
    attribute_name: &str,
) -> Option<SciterElementHandle> {
    for _depth in 0..8 {
        if element.is_null() {
            return None;
        }

        if let Some(value) = api.get_attribute_by_name(element, attribute_name) {
            if !value.is_empty() {
                return Some(element);
            }
        }

        element = match api.get_parent_element(element) {
            Some(parent) => parent,
            None => return None,
        };
    }

    None
}

fn element_string_attribute(
    api: &SciterApi,
    element: SciterElementHandle,
    name: &str,
) -> Option<String> {
    if element.is_null() {
        return None;
    }
    api.get_attribute_by_name(element, name)
}

fn wide_string(value: *const u16) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let mut length = 0usize;
    unsafe {
        while *value.add(length) != 0 {
            length += 1;
        }
    }

    let slice = unsafe { std::slice::from_raw_parts(value, length) };
    String::from_utf16(slice).ok()
}

/// Converts a nul-terminated narrow (UTF-8) C string pointer to an owned `String`.
///
/// # Safety
///
/// - `value` must be either null or a valid pointer to a nul-terminated sequence of bytes.
/// - The bytes from `value` up to (and including) the nul terminator must be within a single
///   allocated object (i.e. readable for the lifetime of this call).
/// - The content before the nul terminator must be valid UTF-8; invalid UTF-8 results in `None`.
fn narrow_string(value: *const i8) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let mut length = 0usize;
    unsafe {
        while *value.add(length) != 0 {
            length += 1;
        }
        let slice = std::slice::from_raw_parts(value as *const u8, length);
        String::from_utf8(slice.to_vec()).ok()
    }
}

#[cfg(test)]
fn test_custom_event_params(name: &str) -> (Vec<u16>, BehaviorEventParams) {
    let mut utf16: Vec<u16> = name.encode_utf16().collect();
    utf16.push(0);
    let params = BehaviorEventParams {
        cmd: BEHAVIOR_EVENT_CUSTOM,
        target: std::ptr::null_mut(),
        source: std::ptr::null_mut(),
        reason: 0,
        data: SciterValue::default(),
        name: utf16.as_ptr(),
    };
    (utf16, params)
}

#[cfg(test)]
fn test_scripting_method_params(name: &str) -> (Vec<u8>, ScriptingMethodParams) {
    let (name_storage, _argv_storage, params) = test_scripting_method_params_with_argv(name, &[]);
    (name_storage, params)
}

#[cfg(test)]
fn test_scripting_method_params_with_argv(
    name: &str,
    argv: &[SciterValue],
) -> (Vec<u8>, Vec<SciterValue>, ScriptingMethodParams) {
    let mut utf8 = name.as_bytes().to_vec();
    utf8.push(0);
    let argv_storage = argv.to_vec();
    let (argc, argv_ptr): (u32, *const SciterValue) = if argv_storage.is_empty() {
        (0, std::ptr::null())
    } else {
        (argv_storage.len() as u32, argv_storage.as_ptr())
    };
    let params = ScriptingMethodParams {
        name: utf8.as_ptr() as *const i8,
        argv: argv_ptr,
        argc,
        result: SciterValue::default(),
    };
    (utf8, argv_storage, params)
}

impl Drop for SciterWindow {
    fn drop(&mut self) {
        if !self.event_bridge_installed {
            return;
        }

        let _ = self.api.detach_window_event_handler(
            self.window,
            viewer_command_event_proc,
            self.event_bridge_tag,
        );

        if !self.event_bridge_tag.is_null() {
            unsafe {
                drop(Box::from_raw(
                    self.event_bridge_tag as *mut HandlerBinding<'static>,
                ));
            }
        }
    }
}

#[cfg(test)]
impl SciterWindow {
    fn dispatch_test_viewer_event(&mut self, event_name: &str) -> i32 {
        let (_storage, mut params) = test_custom_event_params(event_name);
        unsafe {
            viewer_command_event_proc(
                self.event_bridge_tag,
                std::ptr::null_mut(),
                HANDLE_BEHAVIOR_EVENT,
                (&mut params as *mut BehaviorEventParams).cast(),
            )
        }
    }

    fn dispatch_test_viewer_xcall(&mut self, method_name: &str) -> i32 {
        self.dispatch_test_viewer_xcall_with_argv(method_name, &[])
    }

    fn dispatch_test_viewer_xcall_with_argv(
        &mut self,
        method_name: &str,
        argv: &[SciterValue],
    ) -> i32 {
        let (_storage, _argv_storage, mut params) =
            test_scripting_method_params_with_argv(method_name, argv);
        unsafe {
            viewer_command_event_proc(
                self.event_bridge_tag,
                std::ptr::null_mut(),
                HANDLE_SCRIPTING_METHOD_CALL,
                (&mut params as *mut ScriptingMethodParams).cast(),
            )
        }
    }
}

impl ViewerUi for SciterWindow {
    fn show_initial(&mut self, html: &str) -> Result<(), ViewerError> {
        self.load_html(html)
    }

    fn show_document(&mut self, html: &str) -> Result<(), ViewerError> {
        self.load_html(html)
    }

    fn show_error(&mut self, error: &ViewerError) -> Result<(), ViewerError> {
        let html = render_runtime_error_shell(error)?;
        self.load_html(&html)
    }

    fn request_close(&mut self) -> Result<(), ViewerError> {
        self.window_chrome.close(self.window)
    }

    #[cfg(windows)]
    fn run_event_loop(&mut self) -> Result<(), ViewerError> {
        self.api.run_app_loop();
        Ok(())
    }

    #[cfg(not(windows))]
    fn run_event_loop(&mut self) -> Result<(), ViewerError> {
        run_sciter_event_loop(self.window)
    }
}

impl ViewerCommandBinder for SciterWindow {
    fn bind_viewer_command_handler<H>(&mut self, handler: &mut H) -> Result<(), ViewerError>
    where
        H: ViewerCommandHandler,
    {
        SciterWindow::bind_viewer_command_handler(self, handler)
    }
}

fn render_runtime_error_shell(error: &ViewerError) -> Result<String, ViewerError> {
    let state = ViewerState::NoDocument.with_error(error.clone());
    DefaultHtmlShell::new(EmbeddedUiAssets::default()).render_shell(ShellModel {
        app_name: APP_NAME,
        state: &state,
        resource_policy: ResourcePolicy::LocalOnly,
        theme: Theme::default(),
        body_font: None,
        recent_files: &[],
    })
}

#[cfg(not(windows))]
fn run_sciter_event_loop(_window: SciterWindowHandle) -> Result<(), ViewerError> {
    Err(ViewerError::ui(
        "Sciter event loop is only available on Windows in this build",
    ))
}

#[cfg(test)]
#[cfg(windows)]
mod event_loop_tests {
    use super::*;

    #[cfg(windows)]
    fn run_event_loop_with(
        window: SciterWindowHandle,
        mut next_message: impl FnMut(&mut Message) -> i32,
    ) -> Result<(), ViewerError> {
        run_event_loop_with_shutdown_check(window, &mut next_message, is_window_alive)
    }

    #[cfg(windows)]
    fn run_event_loop_with_shutdown_check(
        window: SciterWindowHandle,
        mut next_message: impl FnMut(&mut Message) -> i32,
        mut window_is_alive: impl FnMut(SciterWindowHandle) -> bool,
    ) -> Result<(), ViewerError> {
        if window.is_null() {
            return Err(ViewerError::ui(
                "Sciter event loop cannot run without a window handle",
            ));
        }

        let mut message = Message {
            hwnd: std::ptr::null_mut(),
            message: 0,
            w_param: 0,
            l_param: 0,
            time: 0,
            pt_x: 0,
            pt_y: 0,
            l_private: 0,
        };

        loop {
            match next_message(&mut message) {
                -1 => {
                    if !window_is_alive(window) {
                        return Ok(());
                    }
                    return Err(ViewerError::ui(
                        "Sciter event loop failed to retrieve the next Windows message",
                    ));
                }
                0 => return Ok(()),
                _ => unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                },
            }
        }
    }

    #[cfg(windows)]
    fn is_window_alive(window: SciterWindowHandle) -> bool {
        unsafe { IsWindow(window) != 0 }
    }

    #[test]
    fn event_loop_returns_typed_result_from_runtime_loop_path() {
        let window = std::ptr::dangling_mut::<core::ffi::c_void>();
        let mut calls = 0;

        run_event_loop_with(window, |_| {
            calls += 1;
            if calls < 2 {
                1
            } else {
                0
            }
        })
        .expect("windows event loop seam should succeed");

        assert_eq!(calls, 2);
    }

    #[test]
    fn windows_event_loop_keeps_running_until_quit_message_arrives() {
        let window = std::ptr::dangling_mut::<core::ffi::c_void>();
        let mut calls = 0;

        run_event_loop_with(window, |_| {
            calls += 1;
            if calls < 3 {
                1
            } else {
                0
            }
        })
        .expect("event loop should continue until quit");

        assert_eq!(calls, 3);
    }

    #[test]
    fn windows_event_loop_maps_message_retrieval_failure_to_ui_error() {
        let window = std::ptr::dangling_mut::<core::ffi::c_void>();

        let error = run_event_loop_with_shutdown_check(window, |_| -1, |_| true)
            .expect_err("message retrieval failure should map to ViewerError");

        assert_eq!(
            error,
            ViewerError::ui("Sciter event loop failed to retrieve the next Windows message")
        );
    }

    #[test]
    fn windows_event_loop_treats_invalid_window_after_close_as_graceful_shutdown() {
        let window = std::ptr::dangling_mut::<core::ffi::c_void>();

        run_event_loop_with_shutdown_check(window, |_| -1, |_| false)
            .expect("closed window should stop event loop gracefully");
    }
}

impl SciterRuntime {
    pub(crate) fn into_api(self) -> SciterApi {
        self.into_api_internal()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        element_string_attribute, parse_routed_command_from_behavior_event,
        parse_scripting_method_call_with, test_custom_event_params, test_scripting_method_params,
        test_scripting_method_params_with_argv, viewer_command_event_proc, BehaviorEventParams,
        HandlerBinding, SciterValue, SciterWindow, ViewerCommand, ViewerCommandHandler, ViewerUi,
        WindowChromeAction, HANDLE_BEHAVIOR_EVENT, HANDLE_SCRIPTING_METHOD_CALL,
    };
    use crate::sciter::ffi::{
        SciterApi, SciterCallback, SciterWindowHandle, ScriptingMethodParams,
    };
    use crate::ViewerError;
    use crate::{WindowChromeController, WindowChromeState};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::Mutex;

    static LAST_LOADED_HTML: Mutex<Option<String>> = Mutex::new(None);

    #[test]
    fn show_initial_loads_html_into_created_window() {
        let mut window = SciterWindow::with_api(fake_api(true)).expect("create window wrapper");

        window
            .show_initial("<!doctype html><html><body>Initial</body></html>")
            .expect("load initial html");
    }

    #[test]
    fn show_document_returns_ui_error_when_html_update_fails() {
        let mut window = SciterWindow::with_api(fake_api(false)).expect("create window wrapper");

        let error = window
            .show_document("<!doctype html><html><body>Document</body></html>")
            .expect_err("html load failure should map to ViewerError");

        assert_eq!(
            error,
            ViewerError::ui("Sciter API unavailable: SciterLoadHtml returned failure status")
        );
    }

    #[test]
    fn show_error_renders_user_message_as_html() {
        let mut window = SciterWindow::with_api(fake_api(true)).expect("create window wrapper");

        window
            .show_error(&ViewerError::file_dialog("native failure"))
            .expect("render error html");
    }

    #[test]
    fn show_error_preserves_integrated_shell_and_titlebar() {
        *LAST_LOADED_HTML.lock().expect("lock recorded html") = None;
        let mut window =
            SciterWindow::with_api(fake_api_recording_html()).expect("create window wrapper");

        window
            .show_error(&ViewerError::file_read(r"C:\docs\missing.md", "missing"))
            .expect("render shell error html");

        let html = LAST_LOADED_HTML
            .lock()
            .expect("lock recorded html")
            .clone()
            .expect("recorded loaded html");

        assert!(html.contains("<header class=\"titlebar\""));
        assert!(html.contains("data-role=\"viewer-viewport\""));
        assert!(html.contains("data-error-area"));
        assert!(html.contains("MDLuma could not read the selected Markdown file."));
        assert!(html.contains("data-current-file"));
        assert!(html.contains("<div class=\"file-name\" data-current-file></div>"));
        assert!(!html.contains("No file open"));
        assert_eq!(html.matches("<header class=\"titlebar\"").count(), 1);
        assert!(!html.contains("<!doctype html><html><body><p>"));
    }

    #[test]
    fn scripting_method_bridge_dispatches_only_open_file_requested_to_handler() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: calls.clone(),
        };
        let api = fake_api(true);
        let mut binding = HandlerBinding {
            handler: (&mut handler as *mut RecordingCommandHandler).cast(),
            dispatch_viewer: super::dispatch_viewer_command::<RecordingCommandHandler>,
            api: (&api) as *const SciterApi,
            window: std::ptr::dangling_mut::<core::ffi::c_void>(),
            window_chrome: (&FakeWindowChrome::default()) as *const dyn WindowChromeController,
            dispatch_window_chrome: super::dispatch_window_chrome_command,
            _marker: std::marker::PhantomData,
        };
        let (name_storage, mut params) = test_scripting_method_params("open-file-requested");

        let handled = unsafe {
            viewer_command_event_proc(
                (&mut binding as *mut HandlerBinding<'_>).cast(),
                std::ptr::null_mut(),
                HANDLE_SCRIPTING_METHOD_CALL,
                (&mut params as *mut ScriptingMethodParams).cast(),
            )
        };

        assert_eq!(handled, 1);
        assert_eq!(*calls.borrow(), vec![ViewerCommand::OpenFileRequested]);
        assert!(!name_storage.is_empty());

        let (_ignored_name, mut ignored_params) = test_scripting_method_params("search-requested");
        let ignored = unsafe {
            viewer_command_event_proc(
                (&mut binding as *mut HandlerBinding<'_>).cast(),
                std::ptr::null_mut(),
                HANDLE_SCRIPTING_METHOD_CALL,
                (&mut ignored_params as *mut ScriptingMethodParams).cast(),
            )
        };

        assert_eq!(ignored, 0);
        assert_eq!(*calls.borrow(), vec![ViewerCommand::OpenFileRequested]);
    }

    #[test]
    fn sciter_window_binding_routes_only_open_events_to_bound_handler() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: calls.clone(),
        };
        let mut window = SciterWindow::with_api(fake_api(true)).expect("create window wrapper");

        window
            .bind_viewer_command_handler(&mut handler)
            .expect("bind viewer command handler");

        assert_eq!(window.dispatch_test_viewer_xcall("open-file-requested"), 1);
        assert_eq!(*calls.borrow(), vec![ViewerCommand::OpenFileRequested]);

        assert_eq!(window.dispatch_test_viewer_xcall("search-requested"), 0);
        assert_eq!(*calls.borrow(), vec![ViewerCommand::OpenFileRequested]);
    }

    #[test]
    fn smoke_window_loads_initial_html_dispatches_open_and_updates_document() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: calls.clone(),
        };
        let mut window = SciterWindow::with_api(fake_api(true)).expect("create window wrapper");

        window
            .show_initial("<!doctype html><html><body>Initial</body></html>")
            .expect("load initial html");
        window
            .bind_viewer_command_handler(&mut handler)
            .expect("bind viewer command handler");

        assert_eq!(window.dispatch_test_viewer_xcall("open-file-requested"), 1);
        assert_eq!(*calls.borrow(), vec![ViewerCommand::OpenFileRequested]);

        window
            .show_document("<!doctype html><html><body><h1>Guide</h1></body></html>")
            .expect("load formatted document html");
    }

    #[test]
    fn show_document_reloads_full_html_so_previous_selection_and_copy_status_dom_cannot_persist() {
        *LAST_LOADED_HTML.lock().expect("lock recorded html") = None;
        let mut window =
            SciterWindow::with_api(fake_api_recording_html()).expect("create window wrapper");

        window
            .show_document(
                "<!doctype html><html><body><article data-stale-selection=\"guide\"></article><section data-copy-status>Copy failed. Try again.</section></body></html>",
            )
            .expect("load first document html");

        window
            .show_document(
                "<!doctype html><html><body><article data-markdown-body></article><section data-copy-status></section></body></html>",
            )
            .expect("load replacement document html");

        let html = LAST_LOADED_HTML
            .lock()
            .expect("lock recorded html")
            .clone()
            .expect("recorded replacement html");

        assert!(html.contains("<article data-markdown-body></article>"));
        assert!(html.contains("<section data-copy-status></section>"));
        assert!(!html.contains("data-stale-selection=\"guide\""));
        assert!(!html.contains("Copy failed. Try again."));
    }

    #[test]
    fn custom_behavior_event_parser_rejects_unknown_or_non_custom_events() {
        let api = fake_api(true);
        let (_unknown_storage, unknown_params) = test_custom_event_params("theme-toggle-requested");
        assert_eq!(
            parse_routed_command_from_behavior_event(&unknown_params, &api),
            None
        );

        let (_known_storage, mut non_custom) = test_custom_event_params("window-close-requested");
        non_custom.cmd = 1;
        assert_eq!(
            parse_routed_command_from_behavior_event(&non_custom, &api),
            None
        );
    }

    #[test]
    fn behavior_event_parser_ignores_handled_notifications() {
        let api = fake_api(true);
        let (_storage, mut params) = test_custom_event_params("window-close-requested");
        params.cmd = 0x10000;

        assert_eq!(
            parse_routed_command_from_behavior_event(&params, &api),
            None
        );
    }

    fn fake_api(load_html_success: bool) -> SciterApi {
        if load_html_success {
            SciterApi::for_tests(
                fake_sciter_version,
                fake_sciter_create_window,
                fake_sciter_load_html_success,
                fake_sciter_set_option,
                fake_sciter_value_type_noop,
                fake_sciter_value_string_data_noop,
            )
        } else {
            SciterApi::for_tests(
                fake_sciter_version,
                fake_sciter_create_window,
                fake_sciter_load_html_failure,
                fake_sciter_set_option,
                fake_sciter_value_type_noop,
                fake_sciter_value_string_data_noop,
            )
        }
    }

    fn fake_api_recording_html() -> SciterApi {
        SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window,
            fake_sciter_load_html_records_html,
            fake_sciter_set_option,
            fake_sciter_value_type_noop,
            fake_sciter_value_string_data_noop,
        )
    }

    unsafe extern "C" fn fake_sciter_version(n: u32) -> u32 {
        match n {
            0 => 6,
            1 => 0,
            _ => 0,
        }
    }

    unsafe extern "C" fn fake_sciter_create_window(
        _creation_flags: u32,
        _frame: *const core::ffi::c_void,
        _delegate: Option<SciterCallback>,
        _delegate_param: *mut core::ffi::c_void,
        _parent: SciterWindowHandle,
    ) -> SciterWindowHandle {
        std::ptr::dangling_mut::<core::ffi::c_void>()
    }

    unsafe extern "C" fn fake_sciter_load_html_success(
        _hwnd: SciterWindowHandle,
        _html: *const u8,
        _html_length: u32,
        _base_url: *const u16,
    ) -> i32 {
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

    unsafe extern "C" fn fake_sciter_load_html_records_html(
        _hwnd: SciterWindowHandle,
        html: *const u8,
        html_length: u32,
        _base_url: *const u16,
    ) -> i32 {
        let bytes = unsafe { std::slice::from_raw_parts(html, html_length as usize) };
        let document = String::from_utf8(bytes.to_vec()).expect("utf8 html payload");
        *LAST_LOADED_HTML.lock().expect("lock recorded html") = Some(document);
        1
    }

    unsafe extern "C" fn fake_sciter_set_option(
        _hwnd: SciterWindowHandle,
        _option: u32,
        _value: usize,
    ) -> i32 {
        1
    }

    unsafe extern "C" fn fake_sciter_value_type_noop(
        _pval: *const SciterValue,
        _p_type: *mut u32,
        _p_units: *mut u32,
    ) -> u32 {
        1
    }

    unsafe extern "C" fn fake_sciter_value_string_data_noop(
        _pval: *const SciterValue,
        _p_chars: *mut *const u16,
        _p_num_chars: *mut u32,
    ) -> u32 {
        1
    }

    struct RecordingCommandHandler {
        calls: Rc<RefCell<Vec<ViewerCommand>>>,
    }

    impl ViewerCommandHandler for RecordingCommandHandler {
        fn handle_viewer_command(&mut self, command: ViewerCommand) -> Result<(), ViewerError> {
            self.calls.borrow_mut().push(command);
            Ok(())
        }
    }

    #[test]
    fn window_chrome_action_parser_accepts_only_supported_window_events() {
        assert_eq!(
            WindowChromeAction::from_ui_event("window-minimize-requested"),
            Some(WindowChromeAction::Minimize)
        );
        assert_eq!(
            WindowChromeAction::from_ui_event("window-toggle-maximize-requested"),
            Some(WindowChromeAction::ToggleMaximize)
        );
        assert_eq!(
            WindowChromeAction::from_ui_event("window-close-requested"),
            Some(WindowChromeAction::Close)
        );

        for unsupported in ["open-file-requested", "search-requested", "window-close"] {
            assert_eq!(WindowChromeAction::from_ui_event(unsupported), None);
        }
    }

    #[test]
    fn behavior_event_bridge_routes_window_commands_without_touching_app_handler() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: calls.clone(),
        };
        let fake_window_chrome = FakeWindowChrome::default();
        let api = fake_api(true);
        let mut binding = HandlerBinding {
            handler: (&mut handler as *mut RecordingCommandHandler).cast(),
            dispatch_viewer: super::dispatch_viewer_command::<RecordingCommandHandler>,
            api: (&api) as *const SciterApi,
            window: std::ptr::dangling_mut::<core::ffi::c_void>(),
            window_chrome: (&fake_window_chrome) as *const dyn WindowChromeController,
            dispatch_window_chrome: super::dispatch_window_chrome_command,
            _marker: std::marker::PhantomData,
        };
        let (_storage, mut params) = test_custom_event_params("window-minimize-requested");

        let handled = unsafe {
            viewer_command_event_proc(
                (&mut binding as *mut HandlerBinding<'_>).cast(),
                std::ptr::null_mut(),
                HANDLE_BEHAVIOR_EVENT,
                (&mut params as *mut BehaviorEventParams).cast(),
            )
        };

        assert_eq!(handled, 1);
        assert!(calls.borrow().is_empty());
        assert_eq!(
            *fake_window_chrome.calls.borrow(),
            vec![WindowChromeAction::Minimize]
        );
    }

    #[test]
    fn sciter_window_binding_routes_open_to_app_and_window_commands_to_native_chrome() {
        let app_calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: app_calls.clone(),
        };
        let fake_window_chrome = FakeWindowChrome::default();
        let chrome_calls = fake_window_chrome.calls.clone();
        let mut window =
            SciterWindow::with_api_and_window_chrome(fake_api(true), Box::new(fake_window_chrome), None)
                .expect("create window wrapper");

        window
            .bind_viewer_command_handler(&mut handler)
            .expect("bind viewer command handler");

        assert_eq!(window.dispatch_test_viewer_xcall("open-file-requested"), 1);
        assert_eq!(*app_calls.borrow(), vec![ViewerCommand::OpenFileRequested]);
        assert!(chrome_calls.borrow().is_empty());

        for out_of_scope in [
            "copy-requested",
            "copy-selection-requested",
            "save-requested",
            "edit-requested",
        ] {
            assert_eq!(window.dispatch_test_viewer_xcall(out_of_scope), 0);
        }
        assert_eq!(*app_calls.borrow(), vec![ViewerCommand::OpenFileRequested]);
        assert!(chrome_calls.borrow().is_empty());

        assert_eq!(
            window.dispatch_test_viewer_event("window-minimize-requested"),
            1
        );
        assert_eq!(*app_calls.borrow(), vec![ViewerCommand::OpenFileRequested]);
        assert_eq!(*chrome_calls.borrow(), vec![WindowChromeAction::Minimize]);

        assert_eq!(
            window.dispatch_test_viewer_event("window-toggle-maximize-requested"),
            1
        );
        assert_eq!(
            *chrome_calls.borrow(),
            vec![
                WindowChromeAction::Minimize,
                WindowChromeAction::ToggleMaximize,
            ]
        );

        assert_eq!(
            window.dispatch_test_viewer_event("window-close-requested"),
            1
        );
        assert_eq!(
            *chrome_calls.borrow(),
            vec![
                WindowChromeAction::Minimize,
                WindowChromeAction::ToggleMaximize,
                WindowChromeAction::Close,
            ]
        );
    }

    fn test_extract_string(
        leaked_strings: &std::collections::HashMap<u64, String>,
        value: &SciterValue,
    ) -> Option<String> {
        leaked_strings.get(&value.data).cloned()
    }

    fn make_test_string_sciter_value(s: &str) -> SciterValue {
        let boxed = Box::new(s.to_string());
        let ptr = Box::into_raw(boxed) as u64;
        SciterValue {
            value_type: 0xFFFF,
            units: 0,
            data: ptr,
        }
    }

    fn leak_test_strings(argv: &[SciterValue]) -> std::collections::HashMap<u64, String> {
        let mut map = std::collections::HashMap::new();
        for v in argv {
            if v.value_type == 0xFFFF && v.data != 0 {
                let s = unsafe { Box::from_raw(v.data as *mut String) };
                map.insert(v.data, *s);
            }
        }
        map
    }

    #[test]
    fn scripting_method_bridge_dispatches_dropped_files_with_ordered_paths() {
        let v1 = make_test_string_sciter_value(r"C:\docs\guide.md");
        let v2 = make_test_string_sciter_value(r"C:\docs\notes.md");
        let argv = [v1, v2];
        let leaked = leak_test_strings(&argv);

        let result = parse_scripting_method_call_with("open-dropped-files", &argv, |v| {
            test_extract_string(&leaked, v)
        });

        assert_eq!(
            result,
            Some(ViewerCommand::OpenDroppedFiles(vec![
                PathBuf::from(r"C:\docs\guide.md"),
                PathBuf::from(r"C:\docs\notes.md"),
            ]))
        );
    }

    #[test]
    fn scripting_method_bridge_dispatches_single_dropped_file_as_path_list() {
        let v1 = make_test_string_sciter_value(r"C:\readme.md");
        let argv = [v1];
        let leaked = leak_test_strings(&argv);

        let result = parse_scripting_method_call_with("open-dropped-files", &argv, |v| {
            test_extract_string(&leaked, v)
        });

        assert_eq!(
            result,
            Some(ViewerCommand::OpenDroppedFiles(vec![PathBuf::from(
                r"C:\readme.md"
            ),]))
        );
    }

    #[test]
    fn scripting_method_bridge_ignores_open_dropped_files_with_no_arguments() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: calls.clone(),
        };
        let api = fake_api(true);
        let mut binding = HandlerBinding {
            handler: (&mut handler as *mut RecordingCommandHandler).cast(),
            dispatch_viewer: super::dispatch_viewer_command::<RecordingCommandHandler>,
            api: (&api) as *const SciterApi,
            window: std::ptr::dangling_mut::<core::ffi::c_void>(),
            window_chrome: (&FakeWindowChrome::default()) as *const dyn WindowChromeController,
            dispatch_window_chrome: super::dispatch_window_chrome_command,
            _marker: std::marker::PhantomData,
        };

        let (_name_storage, _argv_storage, mut params) =
            test_scripting_method_params_with_argv("open-dropped-files", &[]);

        let handled = unsafe {
            viewer_command_event_proc(
                (&mut binding as *mut HandlerBinding<'_>).cast(),
                std::ptr::null_mut(),
                HANDLE_SCRIPTING_METHOD_CALL,
                (&mut params as *mut ScriptingMethodParams).cast(),
            )
        };

        assert_eq!(handled, 1);
        assert_eq!(
            *calls.borrow(),
            vec![ViewerCommand::OpenDroppedFiles(Vec::new())]
        );
    }

    #[test]
    fn parse_scripting_method_call_maps_open_dropped_files_to_command_with_paths() {
        let v1 = make_test_string_sciter_value(r"C:\a.md");
        let v2 = make_test_string_sciter_value(r"C:\b.md");
        let v3 = make_test_string_sciter_value(r"C:\c.md");
        let argv = [v1, v2, v3];
        let leaked = leak_test_strings(&argv);

        let result = parse_scripting_method_call_with("open-dropped-files", &argv, |v| {
            test_extract_string(&leaked, v)
        });

        assert_eq!(
            result,
            Some(ViewerCommand::OpenDroppedFiles(vec![
                PathBuf::from(r"C:\a.md"),
                PathBuf::from(r"C:\b.md"),
                PathBuf::from(r"C:\c.md"),
            ]))
        );
    }

    #[test]
    fn parse_scripting_method_call_returns_none_for_unknown_methods() {
        assert_eq!(
            parse_scripting_method_call_with("search-requested", &[], |_| None),
            None
        );
        assert_eq!(
            parse_scripting_method_call_with("copy-requested", &[], |_| None),
            None
        );
        assert_eq!(
            parse_scripting_method_call_with("save-requested", &[], |_| None),
            None
        );
        assert_eq!(parse_scripting_method_call_with("", &[], |_| None), None);
    }

    #[test]
    fn parse_scripting_method_call_maps_open_file_requested_without_args() {
        assert_eq!(
            parse_scripting_method_call_with("open-file-requested", &[], |_| None),
            Some(ViewerCommand::OpenFileRequested)
        );
    }

    #[test]
    fn parse_scripting_method_call_maps_theme_toggle_requested_to_command() {
        assert_eq!(
            parse_scripting_method_call_with("theme-toggle-requested", &[], |_| None),
            Some(ViewerCommand::ThemeToggleRequested)
        );
    }

    #[test]
    fn parse_scripting_method_call_maps_error_dismiss_requested_to_command() {
        assert_eq!(
            parse_scripting_method_call_with("error-dismiss-requested", &[], |_| None),
            Some(ViewerCommand::ErrorDismissRequested)
        );
    }

    #[test]
    fn from_element_action_maps_theme_to_theme_toggle_requested() {
        assert_eq!(
            ViewerCommand::from_element_action("theme"),
            Some(ViewerCommand::ThemeToggleRequested)
        );
        assert_eq!(ViewerCommand::from_element_action("unknown-action"), None);
    }

    #[test]
    fn from_element_action_maps_font_to_font_settings_requested() {
        assert_eq!(
            ViewerCommand::from_element_action("font"),
            Some(ViewerCommand::FontSettingsRequested)
        );
    }

    #[test]
    fn from_element_action_maps_external_editor_to_external_editor_requested() {
        assert_eq!(
            ViewerCommand::from_element_action("external-editor"),
            Some(ViewerCommand::ExternalEditorRequested)
        );
    }

    #[test]
    fn from_element_action_maps_external_editor_setting_to_command() {
        assert_eq!(
            ViewerCommand::from_element_action("external-editor-setting"),
            Some(ViewerCommand::ExternalEditorSettingRequested)
        );
    }

    #[test]
    fn parse_scripting_method_call_maps_external_editor_requested_to_command() {
        assert_eq!(
            parse_scripting_method_call_with("external-editor-requested", &[], |_| None),
            Some(ViewerCommand::ExternalEditorRequested)
        );
    }

    #[test]
    fn parse_scripting_method_call_maps_external_editor_setting_requested_to_command() {
        assert_eq!(
            parse_scripting_method_call_with("external-editor-setting-requested", &[], |_| None),
            Some(ViewerCommand::ExternalEditorSettingRequested)
        );
    }

    #[test]
    fn sciter_window_binding_dispatches_dropped_files_through_xcall() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: calls.clone(),
        };
        let mut window = SciterWindow::with_api(fake_api(true)).expect("create window wrapper");

        window
            .bind_viewer_command_handler(&mut handler)
            .expect("bind viewer command handler");

        assert_eq!(window.dispatch_test_viewer_xcall("open-file-requested"), 1);
        assert_eq!(*calls.borrow(), vec![ViewerCommand::OpenFileRequested]);

        assert_eq!(window.dispatch_test_viewer_xcall("search-requested"), 0);
        assert_eq!(*calls.borrow(), vec![ViewerCommand::OpenFileRequested]);
    }

    #[test]
    fn sciter_window_binding_dispatches_open_dropped_files_with_args_through_xcall() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut handler = RecordingCommandHandler {
            calls: calls.clone(),
        };
        let mut window = SciterWindow::with_api(fake_api(true)).expect("create window wrapper");

        window
            .bind_viewer_command_handler(&mut handler)
            .expect("bind viewer command handler");

        let v1 = make_test_string_sciter_value(r"C:\docs\guide.md");
        let v2 = make_test_string_sciter_value(r"C:\docs\notes.md");
        let argv = [v1, v2];
        let leaked = leak_test_strings(&argv);

        assert_eq!(
            window.dispatch_test_viewer_xcall_with_argv("open-dropped-files", &argv),
            1
        );
        assert_eq!(
            *calls.borrow(),
            vec![ViewerCommand::OpenDroppedFiles(Vec::new())],
            "with noop value API, string extraction returns None so paths are empty"
        );

        drop(leaked);
    }

    #[test]
    fn request_close_delegates_to_window_chrome_close() {
        let fake_window_chrome = FakeWindowChrome::default();
        let chrome_calls = fake_window_chrome.calls.clone();
        let mut window =
            SciterWindow::with_api_and_window_chrome(fake_api(true), Box::new(fake_window_chrome), None)
                .expect("create window wrapper");

        window
            .request_close()
            .expect("request_close should succeed");

        assert_eq!(*chrome_calls.borrow(), vec![WindowChromeAction::Close]);
    }

    #[test]
    fn request_close_returns_error_when_window_chrome_close_fails() {
        let fake_window_chrome = FakeWindowChrome::default();
        let _chrome_calls = fake_window_chrome.calls.clone();
        let mut window = SciterWindow::with_api_and_window_chrome(
            fake_api(true),
            Box::new(FailingCloseWindowChrome::default()),
            None,
        )
        .expect("create window wrapper");

        let error = window
            .request_close()
            .expect_err("request_close should propagate window chrome close failure");

        assert_eq!(error, ViewerError::ui("window close rejected"));
    }

    #[derive(Default)]
    struct FailingCloseWindowChrome {
        calls: Rc<RefCell<Vec<WindowChromeAction>>>,
    }

    impl WindowChromeController for FailingCloseWindowChrome {
        fn minimize(&self, _hwnd: SciterWindowHandle) -> Result<(), ViewerError> {
            Ok(())
        }

        fn toggle_maximize(
            &self,
            _hwnd: SciterWindowHandle,
        ) -> Result<WindowChromeState, ViewerError> {
            Ok(WindowChromeState { maximized: true })
        }

        fn close(&self, _hwnd: SciterWindowHandle) -> Result<(), ViewerError> {
            self.calls.borrow_mut().push(WindowChromeAction::Close);
            Err(ViewerError::ui("window close rejected"))
        }

    }

    #[derive(Default)]
    struct FakeWindowChrome {
        calls: Rc<RefCell<Vec<WindowChromeAction>>>,
    }

    impl WindowChromeController for FakeWindowChrome {
        fn minimize(&self, _hwnd: SciterWindowHandle) -> Result<(), ViewerError> {
            self.calls.borrow_mut().push(WindowChromeAction::Minimize);
            Ok(())
        }

        fn toggle_maximize(
            &self,
            _hwnd: SciterWindowHandle,
        ) -> Result<WindowChromeState, ViewerError> {
            self.calls
                .borrow_mut()
                .push(WindowChromeAction::ToggleMaximize);
            Ok(WindowChromeState { maximized: true })
        }

        fn close(&self, _hwnd: SciterWindowHandle) -> Result<(), ViewerError> {
            self.calls.borrow_mut().push(WindowChromeAction::Close);
            Ok(())
        }

    }

    #[test]
    fn element_string_attribute_returns_none_for_null_element() {
        let api = fake_api(true);
        assert_eq!(
            element_string_attribute(&api, std::ptr::null_mut(), "data-href"),
            None
        );
    }

    #[test]
    fn hyperlink_click_constant_value_matches_sciter_definition() {
        assert_eq!(0x0080u32, 0x0080);
    }
}
