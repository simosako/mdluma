use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fmt;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

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

        Ok(Self { library, api })
    }
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
