use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fmt;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
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
            Self::load_with(
                runtime_path,
                manifest_canonical_path,
                &LibSystemDynamicLoader,
            )
        }
    }

    pub(crate) fn api_table(&self) -> NonNull<bindings::ISciterAPI> {
        self.api
    }

    pub(crate) fn abi_smoke(
        &self,
        manifest: &ArtifactManifest,
    ) -> Result<AbiSmokeResult, RuntimeAbiError> {
        self.abi_smoke_checked(manifest, is_process_main_thread)
    }

    #[cfg(test)]
    pub(crate) fn abi_smoke_with_main_thread_check(
        &self,
        manifest: &ArtifactManifest,
        is_main_thread: impl FnOnce() -> bool,
    ) -> Result<AbiSmokeResult, RuntimeAbiError> {
        self.abi_smoke_checked(manifest, is_main_thread)
    }

    fn abi_smoke_checked(
        &self,
        manifest: &ArtifactManifest,
        is_main_thread: impl FnOnce() -> bool,
    ) -> Result<AbiSmokeResult, RuntimeAbiError> {
        if !is_main_thread() {
            return Err(RuntimeAbiError::NotMainThread);
        }

        // The committed generated type is authoritative for both offsets. No other table entry is
        // read here, and the function slot is checked before the four documented selector calls.
        let api = unsafe { self.api.as_ref() };
        let actual_api_version = api.version;
        let sciter_version = api
            .SciterVersion
            .ok_or(RuntimeAbiError::NullSciterVersion)?;
        let actual_engine_version = [
            unsafe { sciter_version(0) },
            unsafe { sciter_version(1) },
            unsafe { sciter_version(2) },
            unsafe { sciter_version(3) },
        ];

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
        let raw_library = unsafe { loader.open(c_path.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        let library = NonNull::new(raw_library).ok_or_else(|| RuntimeLoadError::LoadFailure {
            path: runtime_path.to_path_buf(),
            diagnostic: unsafe { copied_dlerror(loader, "dlopen failed without dlerror") },
        })?;

        // POSIX requires clearing stale dlerror state before dlsym and checking it afterwards.
        unsafe { loader.error() };
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

        // This is the single unsafe conversion from an untyped dlsym address to the committed
        // Sciter export ABI. The exact signature matches the existing generated-table FFI.
        let sciter_api: SciterApiExport = unsafe { std::mem::transmute(symbol) };
        let api = NonNull::new(unsafe { sciter_api() } as *mut bindings::ISciterAPI)
            .ok_or(RuntimeLoadError::NullApiTable)?;

        Ok(Self {
            library,
            api,
            _main_thread_only: std::marker::PhantomData,
        })
    }
}

fn is_process_main_thread() -> bool {
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
