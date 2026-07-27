#[cfg(test)]
use std::cell::Cell;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fmt;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::model::ArtifactManifest;

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
#[path = "../../src/sciter/generated_sciter_bindings.rs"]
pub(crate) mod bindings;

pub(crate) const RTLD_NOW: c_int = 0x2;
pub(crate) const RTLD_LOCAL: c_int = 0x4;
const SCITER_API_SYMBOL: &[u8] = b"SciterAPI\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeLoadProgress {
    RuntimeLoadEntered,
    RuntimeLoadCompleted,
    SciterApiExportEntered,
    SciterApiExportCompleted,
    ApiTableEntered,
    ApiTableCompleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AbiSmokeProgress {
    ApiVersionEntered,
    ApiVersionCompleted(u32),
    SciterVersionCallEntered,
    SciterVersionCallCompleted([u32; 4]),
}

type SciterApiExport = unsafe extern "C" fn() -> *const bindings::ISciterAPI;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RuntimeLoadError {
    InvalidPath {
        role: &'static str,
        path: PathBuf,
    },
    NonAbsolutePath {
        role: &'static str,
        path: PathBuf,
    },
    NonCanonicalPath {
        role: &'static str,
        path: PathBuf,
        diagnostic: String,
    },
    PathMismatch {
        runtime_path: PathBuf,
        manifest_path: PathBuf,
    },
    LoadFailure {
        path: PathBuf,
        diagnostic: String,
    },
    SymbolResolutionFailure {
        symbol: &'static str,
        diagnostic: String,
    },
    NullApiTable,
}

impl fmt::Display for RuntimeLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RuntimeLoadError {}

pub(crate) trait DynamicLoader {
    unsafe fn open(&self, path: *const c_char, flags: c_int) -> *mut c_void;
    unsafe fn symbol(&self, handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    unsafe fn error(&self) -> *const c_char;
}

struct LibSystemDynamicLoader;

#[link(name = "System")]
extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn pthread_main_np() -> c_int;
}

impl DynamicLoader for LibSystemDynamicLoader {
    unsafe fn open(&self, path: *const c_char, flags: c_int) -> *mut c_void {
        unsafe { dlopen(path, flags) }
    }

    unsafe fn symbol(&self, handle: *mut c_void, symbol: *const c_char) -> *mut c_void {
        unsafe { dlsym(handle, symbol) }
    }

    unsafe fn error(&self) -> *const c_char {
        unsafe { dlerror() }
    }
}

#[derive(Debug)]
pub(crate) struct SciterRuntime {
    #[allow(dead_code)]
    library: NonNull<c_void>,
    api: NonNull<bindings::ISciterAPI>,
    _main_thread_only: std::marker::PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AbiField {
    ApiVersion,
    SciterVersion,
}

const VALIDATED_ABI_FIELDS: [AbiField; 2] = [AbiField::ApiVersion, AbiField::SciterVersion];

type SciterExecFn = unsafe extern "C" fn(
    bindings::UINT,
    bindings::UINT_PTR,
    bindings::UINT_PTR,
) -> bindings::INT_PTR;
type SciterCreateWindowFn = unsafe extern "C" fn(
    bindings::UINT,
    bindings::LPRECT,
    bindings::LPVOID,
    bindings::LPVOID,
    bindings::HWND,
) -> bindings::HWND;
type SciterSetCallbackFn =
    unsafe extern "C" fn(bindings::HWND, bindings::LPSciterHostCallback, bindings::LPVOID);
type SciterLoadHtmlFn = unsafe extern "C" fn(
    bindings::HWND,
    bindings::LPCBYTE,
    bindings::UINT,
    bindings::LPCWSTR,
) -> bindings::SBOOL;
type SciterWindowExecFn = unsafe extern "C" fn(
    bindings::HWND,
    bindings::UINT,
    bindings::UINT_PTR,
    bindings::UINT_PTR,
) -> bindings::INT_PTR;
type SciterSetupDebugOutputFn =
    unsafe extern "C" fn(bindings::HWND, bindings::LPVOID, bindings::DEBUG_OUTPUT_PROC);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecycleEntry {
    SciterExec,
    SciterCreateWindow,
    SciterSetCallback,
    SciterLoadHtml,
    SciterWindowExec,
    SciterSetupDebugOutput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecycleError {
    NotMainThread,
    MissingEntry(LifecycleEntry),
    NullWindow,
    HtmlTooLarge,
    BaseUrlNotTerminated,
    HtmlLoadFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum AppCommand {
    Stop = 0,
    Init = 2,
    Shutdown = 3,
    LoopIteration = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowFlags(u32);

impl WindowFlags {
    pub(crate) const MAIN: Self = Self(1 << 7);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WindowState {
    Closed,
    Hidden,
    Shown,
    Minimized,
    Maximized,
    FullScreen,
    Other(bindings::INT_PTR),
}

impl WindowState {
    const fn raw(self) -> bindings::UINT_PTR {
        match self {
            Self::Closed => 0,
            Self::Hidden => 1,
            Self::Shown => 2,
            Self::Minimized => 3,
            Self::Maximized => 4,
            Self::FullScreen => 5,
            Self::Other(value) => value as bindings::UINT_PTR,
        }
    }

    const fn from_raw(value: bindings::INT_PTR) -> Self {
        match value {
            0 => Self::Closed,
            1 => Self::Hidden,
            2 => Self::Shown,
            3 => Self::Minimized,
            4 => Self::Maximized,
            5 => Self::FullScreen,
            other => Self::Other(other),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowHandle(NonNull<bindings::HWND__>);

impl WindowHandle {
    const fn raw(self) -> bindings::HWND {
        self.0.as_ptr()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LifecycleCallResult {
    raw: bindings::INT_PTR,
    command: Option<AppCommand>,
}

impl LifecycleCallResult {
    pub(crate) const fn raw(self) -> bindings::INT_PTR {
        self.raw
    }

    pub(crate) const fn validated_fields(self) -> &'static [AbiField] {
        &VALIDATED_ABI_FIELDS
    }

    pub(crate) const fn validates_lifecycle_api(self) -> bool {
        false
    }

    pub(crate) const fn shutdown_complete(self) -> Option<ShutdownComplete> {
        match self.command {
            Some(AppCommand::Shutdown) => Some(ShutdownComplete(())),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShutdownComplete(());

pub(crate) struct LifecycleApi {
    sciter_exec: SciterExecFn,
    sciter_create_window: SciterCreateWindowFn,
    sciter_set_callback: SciterSetCallbackFn,
    sciter_load_html: SciterLoadHtmlFn,
    sciter_window_exec: SciterWindowExecFn,
    sciter_setup_debug_output: SciterSetupDebugOutputFn,
    is_main_thread: fn() -> bool,
    _main_thread_only: std::marker::PhantomData<Rc<()>>,
}

pub(crate) struct HostContext {
    destroyed: bool,
    is_main_thread: fn() -> bool,
    _main_thread_only: std::marker::PhantomData<Rc<()>>,
    _pinned: std::marker::PhantomPinned,
}

impl Drop for HostContext {
    fn drop(&mut self) {
        #[cfg(test)]
        HOST_CONTEXT_DROPS.with(|drops| drops.set(drops.get() + 1));
    }
}

pub(crate) struct HostCallbackContext {
    context: Pin<Box<HostContext>>,
}

impl HostCallbackContext {
    pub(crate) fn new() -> Self {
        Self {
            context: Box::pin(HostContext {
                destroyed: false,
                is_main_thread: is_process_main_thread,
                _main_thread_only: std::marker::PhantomData,
                _pinned: std::marker::PhantomPinned,
            }),
        }
    }

    pub(crate) fn stable_address(&self) -> NonNull<c_void> {
        NonNull::from(self.context.as_ref().get_ref()).cast()
    }
}

pub(crate) struct RegisteredHostContext {
    context: Option<Pin<Box<HostContext>>>,
}

impl RegisteredHostContext {
    pub(crate) fn stable_address(&self) -> NonNull<c_void> {
        NonNull::from(self.context.as_ref().unwrap().as_ref().get_ref()).cast()
    }

    pub(crate) fn destroyed(&self) -> Result<bool, LifecycleError> {
        let context = self.context.as_ref().unwrap().as_ref().get_ref();
        if !(context.is_main_thread)() {
            return Err(LifecycleError::NotMainThread);
        }
        Ok(context.destroyed)
    }
}

impl Drop for RegisteredHostContext {
    fn drop(&mut self) {
        let context = self.context.take().unwrap();
        if !context.as_ref().get_ref().destroyed {
            // Sciter still owns the callback pointer. Leaking is safer than a dangling callback.
            std::mem::forget(context);
        }
    }
}

pub(crate) struct DebugContext {
    protocol_prefix: &'static str,
    callback_count: usize,
    is_main_thread: fn() -> bool,
    _main_thread_only: std::marker::PhantomData<Rc<()>>,
    _pinned: std::marker::PhantomPinned,
}

pub(crate) struct DebugCallbackContext {
    context: Pin<Box<DebugContext>>,
}

impl DebugCallbackContext {
    pub(crate) fn new(protocol_prefix: &'static str) -> Self {
        Self {
            context: Box::pin(DebugContext {
                protocol_prefix,
                callback_count: 0,
                is_main_thread: is_process_main_thread,
                _main_thread_only: std::marker::PhantomData,
                _pinned: std::marker::PhantomPinned,
            }),
        }
    }

    pub(crate) fn stable_address(&self) -> NonNull<c_void> {
        NonNull::from(self.context.as_ref().get_ref()).cast()
    }
}

pub(crate) struct RegisteredDebugContext {
    context: Option<Pin<Box<DebugContext>>>,
}

impl RegisteredDebugContext {
    pub(crate) fn stable_address(&self) -> NonNull<c_void> {
        NonNull::from(self.context.as_ref().unwrap().as_ref().get_ref()).cast()
    }

    pub(crate) fn protocol_prefix(&self) -> &'static str {
        self.context
            .as_ref()
            .unwrap()
            .as_ref()
            .get_ref()
            .protocol_prefix
    }

    pub(crate) fn callback_count(&self) -> Result<usize, LifecycleError> {
        let context = self.context.as_ref().unwrap().as_ref().get_ref();
        if !(context.is_main_thread)() {
            return Err(LifecycleError::NotMainThread);
        }
        Ok(context.callback_count)
    }

    pub(crate) fn release_after_shutdown(mut self, _complete: ShutdownComplete) {
        drop(self.context.take());
    }
}

impl Drop for RegisteredDebugContext {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            // The global debug callback can run until shutdown has returned.
            std::mem::forget(context);
        }
    }
}

#[cfg(test)]
thread_local! {
    static HOST_CONTEXT_DROPS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn host_context_drop_count_for_tests() -> usize {
    HOST_CONTEXT_DROPS.with(Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_context_drop_counts_for_tests() {
    HOST_CONTEXT_DROPS.with(|drops| drops.set(0));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThreadContext {
    Main,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AbiSmokeResult {
    actual_api_version: u32,
    expected_api_version: u32,
    actual_engine_version: [u32; 4],
    expected_engine_version: [u32; 4],
    version_call_returned: bool,
    process_architecture: &'static str,
    thread_context: ThreadContext,
}

impl AbiSmokeResult {
    pub(crate) const fn actual_api_version(self) -> u32 {
        self.actual_api_version
    }

    pub(crate) const fn expected_api_version(self) -> u32 {
        self.expected_api_version
    }

    pub(crate) fn api_matches(self) -> bool {
        self.actual_api_version == self.expected_api_version
    }

    pub(crate) const fn actual_engine_version(self) -> [u32; 4] {
        self.actual_engine_version
    }

    pub(crate) const fn expected_engine_version(self) -> [u32; 4] {
        self.expected_engine_version
    }

    pub(crate) fn engine_matches(self) -> bool {
        self.actual_engine_version == self.expected_engine_version
    }

    pub(crate) const fn version_call_returned(self) -> bool {
        self.version_call_returned
    }

    pub(crate) const fn process_architecture(self) -> &'static str {
        self.process_architecture
    }

    pub(crate) const fn thread_context(self) -> ThreadContext {
        self.thread_context
    }

    pub(crate) const fn validated_fields(self) -> &'static [AbiField] {
        &VALIDATED_ABI_FIELDS
    }

    pub(crate) const fn validates_lifecycle_api(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAbiError {
    NotMainThread,
    NullSciterVersion,
}

const SC_ENGINE_DESTROYED: bindings::UINT = 0x05;
const SCITER_WINDOW_SET_STATE: bindings::UINT = 1;
const SCITER_WINDOW_GET_STATE: bindings::UINT = 2;

unsafe extern "C" fn host_callback(
    notification: bindings::LPSCITER_CALLBACK_NOTIFICATION,
    context: bindings::LPVOID,
) -> bindings::UINT {
    if notification.is_null() || context.is_null() {
        return 0;
    }
    let context = unsafe { &mut *context.cast::<HostContext>() };
    if !(context.is_main_thread)() {
        return 0;
    }
    if unsafe { (*notification).code } == SC_ENGINE_DESTROYED {
        context.destroyed = true;
    }
    0
}

unsafe extern "C" fn debug_callback(
    context: bindings::LPVOID,
    _subsystem: bindings::UINT,
    _severity: bindings::UINT,
    _text: bindings::LPCWSTR,
    _text_length: bindings::UINT,
) {
    if context.is_null() {
        return;
    }
    let context = unsafe { &mut *context.cast::<DebugContext>() };
    if (context.is_main_thread)() {
        context.callback_count += 1;
    }
}

impl LifecycleApi {
    fn ensure_main_thread(&self) -> Result<(), LifecycleError> {
        if (self.is_main_thread)() {
            Ok(())
        } else {
            Err(LifecycleError::NotMainThread)
        }
    }

    pub(crate) fn exec(
        &self,
        command: AppCommand,
        p1: bindings::UINT_PTR,
        p2: bindings::UINT_PTR,
    ) -> Result<LifecycleCallResult, LifecycleError> {
        self.ensure_main_thread()?;
        let raw = unsafe { (self.sciter_exec)(command as bindings::UINT, p1, p2) };
        Ok(LifecycleCallResult {
            raw,
            command: Some(command),
        })
    }

    pub(crate) fn create_window(
        &self,
        flags: WindowFlags,
        frame: Option<&mut bindings::tagRECT>,
        parent: Option<WindowHandle>,
    ) -> Result<WindowHandle, LifecycleError> {
        self.ensure_main_thread()?;
        let frame = frame.map_or(std::ptr::null_mut(), |frame| frame);
        let parent = parent.map_or(std::ptr::null_mut(), WindowHandle::raw);
        let window = unsafe {
            (self.sciter_create_window)(
                flags.0,
                frame,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                parent,
            )
        };
        NonNull::new(window)
            .map(WindowHandle)
            .ok_or(LifecycleError::NullWindow)
    }

    pub(crate) fn register_host_callback(
        &self,
        window: WindowHandle,
        mut owner: HostCallbackContext,
    ) -> Result<RegisteredHostContext, LifecycleError> {
        self.ensure_main_thread()?;
        unsafe {
            owner.context.as_mut().get_unchecked_mut().is_main_thread = self.is_main_thread;
            (self.sciter_set_callback)(
                window.raw(),
                Some(host_callback),
                owner.stable_address().as_ptr(),
            );
        }
        Ok(RegisteredHostContext {
            context: Some(owner.context),
        })
    }

    pub(crate) fn load_html(
        &self,
        window: WindowHandle,
        html: &[u8],
        base_url: Option<&[u16]>,
    ) -> Result<LifecycleCallResult, LifecycleError> {
        self.ensure_main_thread()?;
        let length = u32::try_from(html.len()).map_err(|_| LifecycleError::HtmlTooLarge)?;
        let base_url = match base_url {
            Some(url) if url.last() == Some(&0) => url.as_ptr(),
            Some(_) => return Err(LifecycleError::BaseUrlNotTerminated),
            None => std::ptr::null(),
        };
        let loaded =
            unsafe { (self.sciter_load_html)(window.raw(), html.as_ptr(), length, base_url) };
        if loaded == 0 {
            return Err(LifecycleError::HtmlLoadFailed);
        }
        Ok(LifecycleCallResult {
            raw: loaded.into(),
            command: None,
        })
    }

    pub(crate) fn set_window_state(
        &self,
        window: WindowHandle,
        state: WindowState,
        force: bool,
    ) -> Result<LifecycleCallResult, LifecycleError> {
        self.ensure_main_thread()?;
        let raw = unsafe {
            (self.sciter_window_exec)(
                window.raw(),
                SCITER_WINDOW_SET_STATE,
                state.raw(),
                force as bindings::UINT_PTR,
            )
        };
        Ok(LifecycleCallResult { raw, command: None })
    }

    pub(crate) fn window_state(&self, window: WindowHandle) -> Result<WindowState, LifecycleError> {
        self.ensure_main_thread()?;
        let raw = unsafe { (self.sciter_window_exec)(window.raw(), SCITER_WINDOW_GET_STATE, 0, 0) };
        Ok(WindowState::from_raw(raw))
    }

    pub(crate) fn register_debug_output(
        &self,
        window: Option<WindowHandle>,
        mut owner: DebugCallbackContext,
    ) -> Result<RegisteredDebugContext, LifecycleError> {
        self.ensure_main_thread()?;
        unsafe {
            owner.context.as_mut().get_unchecked_mut().is_main_thread = self.is_main_thread;
            (self.sciter_setup_debug_output)(
                window.map_or(std::ptr::null_mut(), WindowHandle::raw),
                owner.stable_address().as_ptr(),
                Some(debug_callback),
            );
        }
        Ok(RegisteredDebugContext {
            context: Some(owner.context),
        })
    }
}

impl SciterRuntime {
    /// Loads only the already-verified manifest runtime. The library is intentionally never
    /// closed, so Sciter code and its API table remain loaded until process termination.
    ///
    /// # Safety
    /// The caller must have verified the fixed runtime's identity and SHA-256 before calling.
    pub(crate) unsafe fn load_absolute(
        runtime_path: &Path,
        manifest_canonical_path: &Path,
    ) -> Result<Self, RuntimeLoadError> {
        unsafe {
            Self::load_absolute_with_progress(runtime_path, manifest_canonical_path, &mut |_| {})
        }
    }

    pub(crate) unsafe fn load_absolute_with_progress(
        runtime_path: &Path,
        manifest_canonical_path: &Path,
        progress: &mut impl FnMut(RuntimeLoadProgress),
    ) -> Result<Self, RuntimeLoadError> {
        unsafe {
            Self::load_with_progress(
                runtime_path,
                manifest_canonical_path,
                &LibSystemDynamicLoader,
                progress,
            )
        }
    }

    pub(crate) fn api_table(&self) -> NonNull<bindings::ISciterAPI> {
        self.api
    }

    pub(crate) fn lifecycle_api(&self) -> Result<LifecycleApi, LifecycleError> {
        self.lifecycle_api_checked(is_process_main_thread)
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_api_with_main_thread_check(
        &self,
        is_main_thread: fn() -> bool,
    ) -> Result<LifecycleApi, LifecycleError> {
        self.lifecycle_api_checked(is_main_thread)
    }

    fn lifecycle_api_checked(
        &self,
        is_main_thread: fn() -> bool,
    ) -> Result<LifecycleApi, LifecycleError> {
        if !is_main_thread() {
            return Err(LifecycleError::NotMainThread);
        }
        let api = unsafe { self.api.as_ref() };
        Ok(LifecycleApi {
            sciter_exec: api
                .SciterExec
                .ok_or(LifecycleError::MissingEntry(LifecycleEntry::SciterExec))?,
            sciter_create_window: api.SciterCreateWindow.ok_or(LifecycleError::MissingEntry(
                LifecycleEntry::SciterCreateWindow,
            ))?,
            sciter_set_callback: api.SciterSetCallback.ok_or(LifecycleError::MissingEntry(
                LifecycleEntry::SciterSetCallback,
            ))?,
            sciter_load_html: api
                .SciterLoadHtml
                .ok_or(LifecycleError::MissingEntry(LifecycleEntry::SciterLoadHtml))?,
            sciter_window_exec: api.SciterWindowExec.ok_or(LifecycleError::MissingEntry(
                LifecycleEntry::SciterWindowExec,
            ))?,
            sciter_setup_debug_output: api.SciterSetupDebugOutput.ok_or(
                LifecycleError::MissingEntry(LifecycleEntry::SciterSetupDebugOutput),
            )?,
            is_main_thread,
            _main_thread_only: std::marker::PhantomData,
        })
    }

    pub(crate) fn abi_smoke(
        &self,
        manifest: &ArtifactManifest,
    ) -> Result<AbiSmokeResult, RuntimeAbiError> {
        self.abi_smoke_with_progress(manifest, &mut |_| {})
    }

    pub(crate) fn abi_smoke_with_progress(
        &self,
        manifest: &ArtifactManifest,
        progress: &mut impl FnMut(AbiSmokeProgress),
    ) -> Result<AbiSmokeResult, RuntimeAbiError> {
        self.abi_smoke_checked(manifest, is_process_main_thread, progress)
    }

    #[cfg(test)]
    pub(crate) fn abi_smoke_with_main_thread_check(
        &self,
        manifest: &ArtifactManifest,
        is_main_thread: impl FnOnce() -> bool,
    ) -> Result<AbiSmokeResult, RuntimeAbiError> {
        self.abi_smoke_checked(manifest, is_main_thread, &mut |_| {})
    }

    fn abi_smoke_checked(
        &self,
        manifest: &ArtifactManifest,
        is_main_thread: impl FnOnce() -> bool,
        progress: &mut impl FnMut(AbiSmokeProgress),
    ) -> Result<AbiSmokeResult, RuntimeAbiError> {
        if !is_main_thread() {
            return Err(RuntimeAbiError::NotMainThread);
        }

        // The committed generated type is authoritative for both offsets. No other table entry is
        // read here, and the function slot is checked before the four documented selector calls.
        progress(AbiSmokeProgress::ApiVersionEntered);
        let api = unsafe { self.api.as_ref() };
        let actual_api_version = api.version;
        progress(AbiSmokeProgress::ApiVersionCompleted(actual_api_version));
        progress(AbiSmokeProgress::SciterVersionCallEntered);
        let sciter_version = unsafe { self.api.as_ref() }
            .SciterVersion
            .ok_or(RuntimeAbiError::NullSciterVersion)?;
        let actual_engine_version = [
            unsafe { sciter_version(0) },
            unsafe { sciter_version(1) },
            unsafe { sciter_version(2) },
            unsafe { sciter_version(3) },
        ];
        progress(AbiSmokeProgress::SciterVersionCallCompleted(
            actual_engine_version,
        ));

        Ok(AbiSmokeResult {
            actual_api_version,
            expected_api_version: manifest.api_version(),
            actual_engine_version,
            expected_engine_version: manifest.engine_version(),
            version_call_returned: true,
            process_architecture: "arm64",
            thread_context: ThreadContext::Main,
        })
    }

    #[cfg(test)]
    pub(crate) unsafe fn from_api_table_for_tests(api: &bindings::ISciterAPI) -> Self {
        Self {
            library: NonNull::dangling(),
            api: NonNull::from(api),
            _main_thread_only: std::marker::PhantomData,
        }
    }

    pub(crate) unsafe fn load_with<L: DynamicLoader>(
        runtime_path: &Path,
        manifest_canonical_path: &Path,
        loader: &L,
    ) -> Result<Self, RuntimeLoadError> {
        unsafe {
            Self::load_with_progress(runtime_path, manifest_canonical_path, loader, &mut |_| {})
        }
    }

    pub(crate) unsafe fn load_with_progress<L: DynamicLoader>(
        runtime_path: &Path,
        manifest_canonical_path: &Path,
        loader: &L,
        progress: &mut impl FnMut(RuntimeLoadProgress),
    ) -> Result<Self, RuntimeLoadError> {
        validate_canonical_absolute(runtime_path, "runtime")?;
        validate_canonical_absolute(manifest_canonical_path, "manifest")?;
        if runtime_path != manifest_canonical_path {
            return Err(RuntimeLoadError::PathMismatch {
                runtime_path: runtime_path.to_path_buf(),
                manifest_path: manifest_canonical_path.to_path_buf(),
            });
        }

        let c_path = CString::new(runtime_path.as_os_str().as_bytes()).map_err(|_| {
            RuntimeLoadError::InvalidPath {
                role: "runtime",
                path: runtime_path.to_path_buf(),
            }
        })?;
        progress(RuntimeLoadProgress::RuntimeLoadEntered);
        let raw_library = unsafe { loader.open(c_path.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        let library = NonNull::new(raw_library).ok_or_else(|| RuntimeLoadError::LoadFailure {
            path: runtime_path.to_path_buf(),
            diagnostic: unsafe { copied_dlerror(loader, "dlopen failed without dlerror") },
        })?;
        progress(RuntimeLoadProgress::RuntimeLoadCompleted);

        // POSIX requires clearing stale dlerror state before dlsym and checking it afterwards.
        unsafe { loader.error() };
        progress(RuntimeLoadProgress::SciterApiExportEntered);
        let symbol = unsafe {
            loader.symbol(
                library.as_ptr(),
                SCITER_API_SYMBOL.as_ptr().cast::<c_char>(),
            )
        };
        let symbol_error = unsafe { loader.error() };
        if symbol.is_null() || !symbol_error.is_null() {
            let diagnostic = if symbol_error.is_null() {
                "dlsym returned null without dlerror".to_owned()
            } else {
                unsafe { CStr::from_ptr(symbol_error) }
                    .to_string_lossy()
                    .into_owned()
            };
            return Err(RuntimeLoadError::SymbolResolutionFailure {
                symbol: "SciterAPI",
                diagnostic,
            });
        }
        progress(RuntimeLoadProgress::SciterApiExportCompleted);

        // This is the single unsafe conversion from an untyped dlsym address to the committed
        // Sciter export ABI. The exact signature matches the existing generated-table FFI.
        let sciter_api: SciterApiExport = unsafe { std::mem::transmute(symbol) };
        progress(RuntimeLoadProgress::ApiTableEntered);
        let api = NonNull::new(unsafe { sciter_api() } as *mut bindings::ISciterAPI)
            .ok_or(RuntimeLoadError::NullApiTable)?;
        progress(RuntimeLoadProgress::ApiTableCompleted);

        Ok(Self {
            library,
            api,
            _main_thread_only: std::marker::PhantomData,
        })
    }
}

pub(crate) fn is_process_main_thread() -> bool {
    unsafe { pthread_main_np() == 1 }
}

fn validate_canonical_absolute(path: &Path, role: &'static str) -> Result<(), RuntimeLoadError> {
    if !path.is_absolute() {
        return Err(RuntimeLoadError::NonAbsolutePath {
            role,
            path: path.to_path_buf(),
        });
    }
    if path.as_os_str().as_bytes().contains(&0) {
        return Err(RuntimeLoadError::InvalidPath {
            role,
            path: path.to_path_buf(),
        });
    }
    let canonical = fs::canonicalize(path).map_err(|error| RuntimeLoadError::NonCanonicalPath {
        role,
        path: path.to_path_buf(),
        diagnostic: error.to_string(),
    })?;
    if canonical != path {
        return Err(RuntimeLoadError::NonCanonicalPath {
            role,
            path: path.to_path_buf(),
            diagnostic: format!("canonical path is {}", canonical.display()),
        });
    }
    Ok(())
}

unsafe fn copied_dlerror<L: DynamicLoader>(loader: &L, fallback: &str) -> String {
    let error = unsafe { loader.error() };
    if error.is_null() {
        fallback.to_owned()
    } else {
        unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned()
    }
}
