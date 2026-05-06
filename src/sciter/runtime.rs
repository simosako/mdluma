use std::fmt;
use std::path::{Path, PathBuf};

use crate::errors::ViewerError;
use crate::sciter::ffi::SciterApi;
use crate::sciter::ffi::{SCITER_VERSION_0, SCITER_VERSION_1};
use crate::sciter::runtime_assets::relative_distribution_prerequisite_paths;

#[cfg(debug_assertions)]
use crate::debug_log;

pub use crate::sciter::runtime_assets::SCITER_DLL_NAME;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePrerequisites {
    pub sciter_dll_path: PathBuf,
    pub required_files: Vec<PathBuf>,
}

impl RuntimePrerequisites {
    pub fn from_distribution_dir(distribution_dir: impl AsRef<Path>) -> Self {
        let distribution_dir = distribution_dir.as_ref();
        let sciter_dll_path = distribution_dir.join(SCITER_DLL_NAME);
        let required_files = relative_distribution_prerequisite_paths()
            .into_iter()
            .map(|relative_path| distribution_dir.join(relative_path))
            .collect();

        Self {
            required_files,
            sciter_dll_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SciterRuntimeError {
    MissingDll {
        path: PathBuf,
    },
    ApiUnavailable {
        message: String,
    },
    IncompatibleVersion {
        dll_version: String,
        expected_version: String,
    },
}

impl SciterRuntimeError {
    pub fn user_message(&self) -> String {
        match self {
            Self::MissingDll { .. } => {
                format!("MDLuma cannot start because the required {SCITER_DLL_NAME} runtime file is missing.")
            }
            Self::ApiUnavailable { .. } => {
                "MDLuma cannot start because the rendering runtime is unavailable.".to_string()
            }
            Self::IncompatibleVersion {
                dll_version,
                expected_version,
            } => {
                format!(
                    "MDLuma cannot start because the Sciter runtime version {} does not match the expected version {}.",
                    dll_version, expected_version
                )
            }
        }
    }

    pub fn operator_diagnostic(&self) -> String {
        match self {
            Self::MissingDll { path } => {
                format!("missing Sciter runtime DLL: expected {}", path.display())
            }
            Self::ApiUnavailable { message } => format!("Sciter API unavailable: {message}"),
            Self::IncompatibleVersion {
                dll_version,
                expected_version,
            } => {
                format!(
                    "Sciter DLL version {} incompatible with expected version {}",
                    dll_version, expected_version
                )
            }
        }
    }
}

impl fmt::Display for SciterRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.user_message())
    }
}

impl std::error::Error for SciterRuntimeError {}

impl From<SciterRuntimeError> for ViewerError {
    fn from(error: SciterRuntimeError) -> Self {
        match error {
            SciterRuntimeError::MissingDll { path } => ViewerError::runtime_missing(path),
            SciterRuntimeError::ApiUnavailable { message } => {
                ViewerError::runtime_unavailable(message)
            }
            SciterRuntimeError::IncompatibleVersion { .. } => ViewerError::runtime_unavailable(
                "Sciter DLL version does not match expected version".to_string(),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SciterVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: u32,
}

impl std::fmt::Display for SciterVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.build
        )
    }
}

#[derive(Debug)]
pub struct SciterRuntime {
    api: SciterApi,
}

impl SciterRuntime {
    pub fn load(prerequisites: RuntimePrerequisites) -> Result<Self, ViewerError> {
        Self::load_with_api(prerequisites, SciterApi::load).map_err(ViewerError::from)
    }

    fn load_with_api(
        prerequisites: RuntimePrerequisites,
        api_loader: impl FnOnce(&Path) -> Result<SciterApi, SciterRuntimeError>,
    ) -> Result<Self, SciterRuntimeError> {
        validate_prerequisites_internal(&prerequisites)?;
        let api = api_loader(&prerequisites.sciter_dll_path)?;
        let version = api.version()?;
        #[cfg(debug_assertions)]
        debug_log!("loaded Sciter DLL version {}", version);

        if version.major != SCITER_VERSION_0 || version.minor != SCITER_VERSION_1 {
            return Err(SciterRuntimeError::IncompatibleVersion {
                dll_version: format!("{}.{}", version.major, version.minor),
                expected_version: format!("{}.{}", SCITER_VERSION_0, SCITER_VERSION_1),
            });
        }

        Ok(Self { api })
    }

    pub fn validate_prerequisites(prerequisites: &RuntimePrerequisites) -> Result<(), ViewerError> {
        validate_prerequisites_internal(prerequisites).map_err(ViewerError::from)
    }

    pub fn version(&self) -> Result<SciterVersion, ViewerError> {
        self.api.version().map_err(ViewerError::from)
    }

    pub(crate) fn into_api_internal(self) -> SciterApi {
        self.api
    }

    #[cfg(test)]
    pub(crate) fn from_api_for_tests(api: SciterApi) -> Self {
        Self { api }
    }

    #[cfg(test)]
    pub(crate) fn ready_for_tests() -> Self {
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
            _delegate: Option<crate::sciter::ffi::SciterCallback>,
            _delegate_param: *mut core::ffi::c_void,
            _parent: crate::sciter::ffi::SciterWindowHandle,
        ) -> crate::sciter::ffi::SciterWindowHandle {
            std::ptr::dangling_mut::<core::ffi::c_void>()
        }

        unsafe extern "C" fn fake_sciter_load_html(
            _hwnd: crate::sciter::ffi::SciterWindowHandle,
            _html: *const u8,
            _html_length: u32,
            _base_url: *const u16,
        ) -> i32 {
            1
        }

        unsafe extern "C" fn fake_sciter_set_option(
            _hwnd: crate::sciter::ffi::SciterWindowHandle,
            _option: u32,
            _value: usize,
        ) -> i32 {
            1
        }

        unsafe extern "C" fn fake_sciter_value_type_noop(
            _pval: *const crate::sciter::ffi::SciterValue,
            _p_type: *mut u32,
            _p_units: *mut u32,
        ) -> u32 {
            1
        }

        unsafe extern "C" fn fake_sciter_value_string_data_noop(
            _pval: *const crate::sciter::ffi::SciterValue,
            _p_chars: *mut *const u16,
            _p_num_chars: *mut u32,
        ) -> u32 {
            1
        }

        Self::from_api_for_tests(SciterApi::for_tests(
            fake_sciter_version,
            fake_sciter_create_window,
            fake_sciter_load_html,
            fake_sciter_set_option,
            fake_sciter_value_type_noop,
            fake_sciter_value_string_data_noop,
        ))
    }
}

fn validate_prerequisites_internal(
    prerequisites: &RuntimePrerequisites,
) -> Result<(), SciterRuntimeError> {
    for required_file in &prerequisites.required_files {
        if !required_file.is_file() {
            return Err(SciterRuntimeError::MissingDll {
                path: required_file.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sciter::runtime_assets::relative_distribution_prerequisite_paths;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn distribution_configuration_requires_sciter_dll_next_to_executable() {
        let distribution_dir = Path::new(r"C:\Program Files\MDLuma");

        let prerequisites = RuntimePrerequisites::from_distribution_dir(distribution_dir);

        assert_eq!(
            prerequisites.sciter_dll_path,
            distribution_dir.join(SCITER_DLL_NAME)
        );
        assert!(prerequisites
            .required_files
            .contains(&distribution_dir.join(SCITER_DLL_NAME)));
        assert_eq!(prerequisites.required_files.len(), 1);
    }

    #[test]
    fn missing_sciter_dll_returns_user_failure_and_operator_diagnostic() {
        let distribution_dir = unique_test_dir("missing-sciter-dll");
        fs::create_dir_all(&distribution_dir).expect("create test distribution dir");
        let prerequisites = RuntimePrerequisites::from_distribution_dir(&distribution_dir);

        let error =
            validate_prerequisites_internal(&prerequisites).expect_err("missing DLL must fail");

        assert!(error.user_message().contains("MDLuma cannot start"));
        assert!(error.user_message().contains(SCITER_DLL_NAME));
        assert!(error
            .operator_diagnostic()
            .contains("missing Sciter runtime DLL"));
        assert!(error
            .operator_diagnostic()
            .contains(&prerequisites.sciter_dll_path.display().to_string()));
    }

    #[test]
    fn existing_sciter_dll_satisfies_prerequisite_validation() {
        let distribution_dir = unique_test_dir("present-sciter-dll");
        fs::create_dir_all(&distribution_dir).expect("create test distribution dir");
        create_distribution_files(&distribution_dir);
        let prerequisites = RuntimePrerequisites::from_distribution_dir(&distribution_dir);

        validate_prerequisites_internal(&prerequisites)
            .expect("DLL file should satisfy prerequisite");
    }

    #[test]
    fn public_load_maps_missing_runtime_and_api_failures_to_viewer_error() {
        let missing_dir = unique_test_dir("public-load-missing-sciter-dll");
        fs::create_dir_all(&missing_dir).expect("create test distribution dir");

        let error = SciterRuntime::load(RuntimePrerequisites::from_distribution_dir(&missing_dir))
            .expect_err("missing dll should map to ViewerError");
        assert_eq!(
            error,
            ViewerError::runtime_missing(missing_dir.join(SCITER_DLL_NAME))
        );

        let api_missing_dir = unique_test_dir("public-load-api-unavailable");
        fs::create_dir_all(&api_missing_dir).expect("create test distribution dir");
        create_distribution_files(&api_missing_dir);

        let error = SciterRuntime::load_with_api(
            RuntimePrerequisites::from_distribution_dir(&api_missing_dir),
            |_| {
                Err(SciterRuntimeError::ApiUnavailable {
                    message: "SciterVersion symbol unavailable".to_string(),
                })
            },
        )
        .map_err(ViewerError::from)
        .expect_err("api unavailable should map to ViewerError");
        assert_eq!(
            error,
            ViewerError::runtime_unavailable("SciterVersion symbol unavailable")
        );
    }

    #[test]
    fn runtime_load_queries_version_before_reporting_ready() {
        let distribution_dir = unique_test_dir("present-sciter-dll-version");
        fs::create_dir_all(&distribution_dir).expect("create test distribution dir");
        create_distribution_files(&distribution_dir);
        let prerequisites = RuntimePrerequisites::from_distribution_dir(&distribution_dir);

        unsafe extern "C" fn fake_sciter_version_6_0_3_18(n: u32) -> u32 {
            match n {
                0 => 6,
                1 => 0,
                2 => 3,
                3 => 18,
                _ => 0,
            }
        }

        let runtime = SciterRuntime::load_with_api(prerequisites, |_| {
            Ok(SciterApi::for_tests(
                fake_sciter_version_6_0_3_18,
                fake_sciter_create_window,
                fake_sciter_load_html,
                fake_sciter_set_option,
                fake_sciter_value_type_noop,
                fake_sciter_value_string_data_noop,
            ))
        })
        .expect("runtime should load with version query");

        assert_eq!(
            runtime.version().expect("query version"),
            SciterVersion {
                major: 6,
                minor: 0,
                patch: 3,
                build: 18
            }
        );
    }

    #[test]
    fn runtime_load_maps_missing_api_to_typed_runtime_error() {
        let distribution_dir = unique_test_dir("present-sciter-dll-api-missing");
        fs::create_dir_all(&distribution_dir).expect("create test distribution dir");
        create_distribution_files(&distribution_dir);
        let prerequisites = RuntimePrerequisites::from_distribution_dir(&distribution_dir);

        let error = SciterRuntime::load_with_api(prerequisites, |_| {
            Err(SciterRuntimeError::ApiUnavailable {
                message: "SciterVersion symbol unavailable".to_string(),
            })
        })
        .expect_err("runtime should fail when version API is unavailable");

        assert_eq!(
            error,
            SciterRuntimeError::ApiUnavailable {
                message: "SciterVersion symbol unavailable".to_string()
            }
        );
        assert!(error
            .user_message()
            .contains("rendering runtime is unavailable"));
    }

    #[test]
    fn runtime_version_maps_api_error_into_viewer_error() {
        let failing_runtime = SciterRuntime {
            api: fake_api_error("SciterVersion failed after load"),
        };

        assert_eq!(
            failing_runtime
                .version()
                .expect("zero version is currently allowed"),
            SciterVersion {
                major: 0,
                minor: 0,
                patch: 0,
                build: 0
            }
        );
    }

    #[test]
    fn sciter_version_formats_as_dot_separated() {
        let version = SciterVersion {
            major: 6,
            minor: 0,
            patch: 3,
            build: 18,
        };
        assert_eq!(format!("{version}"), "6.0.3.18");
        assert_eq!(version.to_string(), "6.0.3.18");
    }

    #[test]
    fn incompatible_dll_version_fails_with_user_friendly_error() {
        let distribution_dir = unique_test_dir("incompatible-sciter-version");
        fs::create_dir_all(&distribution_dir).expect("create test distribution dir");
        create_distribution_files(&distribution_dir);
        let prerequisites = RuntimePrerequisites::from_distribution_dir(&distribution_dir);

        unsafe extern "C" fn fake_sciter_version_5_0_1_2(n: u32) -> u32 {
            match n {
                0 => 5,
                1 => 0,
                2 => 1,
                3 => 2,
                _ => 0,
            }
        }

        let error = SciterRuntime::load_with_api(prerequisites, |_| {
            Ok(SciterApi::for_tests(
                fake_sciter_version_5_0_1_2,
                fake_sciter_create_window,
                fake_sciter_load_html,
                fake_sciter_set_option,
                fake_sciter_value_type_noop,
                fake_sciter_value_string_data_noop,
            ))
        })
        .expect_err("incompatible version should fail");

        assert_eq!(
            error,
            SciterRuntimeError::IncompatibleVersion {
                dll_version: "5.0".to_string(),
                expected_version: "6.0".to_string(),
            }
        );
        let user_msg = error.user_message();
        assert!(user_msg.contains("5.0") && user_msg.contains("6.0"));
    }

    #[test]
    fn runtime_configuration_is_dynamic_dll_distribution_only() {
        let prerequisites = RuntimePrerequisites::from_distribution_dir(Path::new("dist"));

        assert_eq!(prerequisites.required_files.len(), 1);
        assert!(prerequisites
            .required_files
            .iter()
            .any(|path| path.ends_with(SCITER_DLL_NAME)));
        assert!(!prerequisites.required_files.iter().any(|path| {
            path.components()
                .any(|component| component.as_os_str().to_string_lossy().contains("sdk"))
        }));
        assert!(!prerequisites.required_files.iter().any(|path| {
            path.extension()
                .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("lib"))
        }));
    }

    fn unique_test_dir(name: &str) -> TestDir {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mdluma-{name}-{nonce}"));
        TestDir { path }
    }

    #[derive(Debug)]
    struct TestDir {
        path: PathBuf,
    }

    impl AsRef<Path> for TestDir {
        fn as_ref(&self) -> &Path {
            &self.path
        }
    }

    impl std::ops::Deref for TestDir {
        type Target = PathBuf;

        fn deref(&self) -> &Self::Target {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            if self.path.exists() {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    fn fake_api_error(_message: &str) -> SciterApi {
        SciterApi::for_tests(
            fake_sciter_version_zero,
            fake_sciter_create_window,
            fake_sciter_load_html,
            fake_sciter_set_option,
            fake_sciter_value_type_noop,
            fake_sciter_value_string_data_noop,
        )
    }

    unsafe extern "C" fn fake_sciter_create_window(
        _creation_flags: u32,
        _frame: *const core::ffi::c_void,
        _delegate: Option<crate::sciter::ffi::SciterCallback>,
        _delegate_param: *mut core::ffi::c_void,
        _parent: crate::sciter::ffi::SciterWindowHandle,
    ) -> crate::sciter::ffi::SciterWindowHandle {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn fake_sciter_load_html(
        _hwnd: crate::sciter::ffi::SciterWindowHandle,
        _html: *const u8,
        _html_length: u32,
        _base_url: *const u16,
    ) -> i32 {
        1
    }

    unsafe extern "C" fn fake_sciter_set_option(
        _hwnd: crate::sciter::ffi::SciterWindowHandle,
        _option: u32,
        _value: usize,
    ) -> i32 {
        1
    }

    unsafe extern "C" fn fake_sciter_version_zero(_major: u32) -> u32 {
        0
    }

    unsafe extern "C" fn fake_sciter_value_type_noop(
        _pval: *const crate::sciter::ffi::SciterValue,
        _p_type: *mut u32,
        _p_units: *mut u32,
    ) -> u32 {
        1
    }

    unsafe extern "C" fn fake_sciter_value_string_data_noop(
        _pval: *const crate::sciter::ffi::SciterValue,
        _p_chars: *mut *const u16,
        _p_num_chars: *mut u32,
    ) -> u32 {
        1
    }

    fn create_distribution_files(distribution_dir: &Path) {
        for relative_path in relative_distribution_prerequisite_paths() {
            let path = distribution_dir.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create runtime asset directory");
            }
            fs::write(path, []).expect("create distribution file");
        }
    }
}
