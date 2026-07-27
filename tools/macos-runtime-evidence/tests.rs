use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::artifact::{
    parse_minimum_versions, probe_artifact_with_runner, probe_artifact_with_runner_at,
    CodeSigningState, CommandCapture, CommandRunner, CommandStatus, HeaderProbeOutcome,
    SystemCommandRunner,
};
use crate::manifest::{parse_artifact_manifest, parse_license_evidence};
use crate::model::{
    validate_criterion_results, CriterionId, CriterionResult, CriterionStatus, CycleKind,
    CyclePhase, DecisionState, EvidenceError, GateId, GateResult, GateStatus, HarnessEvent,
    PermissionStatus, RunId, ALL_CRITERIA, ALL_GATES, SOURCE_CRITERIA,
};
use crate::sciter::{DynamicLoader, RuntimeLoadError, SciterRuntime, RTLD_LOCAL, RTLD_NOW};

const ARTIFACT_MANIFEST: &str = "schema_version=1\nrepository=https://gitlab.com/sciter-engine/sciter-js-sdk\ncommit=e31ec0f726bdbe5d0402ad647f3b34feef84654e\nsdk_relative_path=bin/macosx/libsciter.dylib\nworkspace_relative_path=vendor/sciter-js-sdk-main/bin/macosx/libsciter.dylib\nsha256=be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b\nengine_version=6.0.3.18\napi_version=10\nversion_header_path=vendor/sciter-js-sdk-main/include/sciter-version.h\napi_header_path=vendor/sciter-js-sdk-main/include/sciter-x-api.h\nversion_header_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-version.h\napi_header_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-x-api.h\n";
const LICENSE_EVIDENCE: &str = "schema_version=1\nredistribution=unresolved\nresigning=unresolved\nlicense_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/LICENSE\neula_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md\npermission_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md\nrequired_about_text=This application uses Sciter Engine (http://sciter.com/), copyright Terra Informatica Software, Inc.\nrequired_distribution_files=LICENSE,SCITER-ENGINE-EULA.md\n";
const EXPECTED_SHA256: &str = "be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b";

#[derive(Default)]
struct FixtureCommandRunner {
    captures: RefCell<Vec<CommandCapture>>,
    calls: RefCell<Vec<(String, Vec<OsString>)>>,
}

impl FixtureCommandRunner {
    fn successful() -> Self {
        let mut runner = Self::default();
        runner.captures.get_mut().extend([
            capture(
                "shasum",
                &format!("{EXPECTED_SHA256}  runtime\n"),
                "shasum stderr\n",
                0,
            ),
            capture("lipo", "x86_64 arm64\n", "lipo stderr\n", 0),
            capture(
                "otool",
                "runtime (architecture x86_64):\n      cmd LC_BUILD_VERSION\n    minos 11.5\n     tool 3\n  version 1266.8\nruntime (architecture arm64):\n      cmd LC_BUILD_VERSION\n    minos 11.5\n     tool 3\n  version 1266.8\n",
                "otool load stderr\n",
                0,
            ),
            capture(
                "otool",
                "runtime (architecture x86_64):\n\t/usr/local/lib/libsciter.dylib (compatibility version 1.0.0, current version 1.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\nruntime (architecture arm64):\n\t/usr/local/lib/libsciter.dylib (compatibility version 1.0.0, current version 1.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\n",
                "otool dependencies stderr\n",
                0,
            ),
            capture(
                "otool",
                "runtime (architecture x86_64):\n/usr/local/lib/libsciter.dylib\nruntime (architecture arm64):\n/usr/local/lib/libsciter.dylib\n",
                "otool install name stderr\n",
                0,
            ),
            capture(
                "codesign",
                "codesign stdout\n",
                "Executable=/tmp/runtime\nSignature=adhoc\nTeamIdentifier=not set\n",
                0,
            ),
        ]);
        runner
    }

    fn calls(&self) -> Vec<(String, Vec<OsString>)> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for FixtureCommandRunner {
    fn run(&self, program: &str, arguments: &[OsString]) -> CommandCapture {
        self.calls
            .borrow_mut()
            .push((program.to_owned(), arguments.to_vec()));
        let mut capture = self.captures.borrow_mut().remove(0);
        capture.program = program.to_owned();
        capture.arguments = arguments.to_vec();
        capture
    }
}

fn capture(program: &str, stdout: &str, stderr: &str, code: i32) -> CommandCapture {
    CommandCapture {
        program: program.to_owned(),
        arguments: Vec::new(),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
        status: CommandStatus::Exited(code),
    }
}

fn artifact_fixture(label: &str) -> (TestDirectory, PathBuf) {
    let test_dir = TestDirectory::new(label);
    let runtime = test_dir
        .path
        .join("vendor/sciter-js-sdk-main/bin/macosx/libsciter.dylib");
    fs::create_dir_all(runtime.parent().unwrap()).unwrap();
    fs::write(&runtime, b"fixture runtime").unwrap();
    (test_dir, runtime)
}

fn write_header_fixture(repository: &TestDirectory, version: [u32; 4], api: u32) {
    let include = repository.path.join("vendor/sciter-js-sdk-main/include");
    fs::create_dir_all(&include).unwrap();
    fs::write(
        include.join("sciter-version.h"),
        format!(
            "#define SCITER_VERSION_0 {}\n#define SCITER_VERSION_1 {}\n#define SCITER_VERSION_2 {}\n#define SCITER_VERSION_3 {}\n",
            version[0], version[1], version[2], version[3]
        ),
    )
    .unwrap();
    fs::write(
        include.join("sciter-x-api.h"),
        format!("#define SCITER_API_VERSION {api}\n"),
    )
    .unwrap();
    let bindings = repository.path.join("src/sciter");
    fs::create_dir_all(&bindings).unwrap();
    fs::write(
        bindings.join("generated_sciter_bindings.rs"),
        "pub const SCITER_VERSION_0: u32 = 6;\npub const SCITER_VERSION_1: u32 = 0;\npub const SCITER_VERSION_2: u32 = 3;\npub const SCITER_VERSION_3: u32 = 18;\npub const SCITER_API_VERSION: u32 = 10;\n",
    )
    .unwrap();
}

fn fixed_manifest() -> crate::model::ArtifactManifest {
    parse_artifact_manifest(ARTIFACT_MANIFEST).unwrap()
}

#[derive(Default)]
struct FixtureDynamicLoader {
    open_result: *mut std::ffi::c_void,
    symbol_result: *mut std::ffi::c_void,
    errors: RefCell<Vec<Option<std::ffi::CString>>>,
    opens: RefCell<Vec<(Vec<u8>, i32)>>,
    symbols: RefCell<Vec<Vec<u8>>>,
}

impl DynamicLoader for FixtureDynamicLoader {
    unsafe fn open(&self, path: *const std::ffi::c_char, flags: i32) -> *mut std::ffi::c_void {
        let path = unsafe { std::ffi::CStr::from_ptr(path) };
        self.opens
            .borrow_mut()
            .push((path.to_bytes().to_vec(), flags));
        self.open_result
    }

    unsafe fn symbol(
        &self,
        _handle: *mut std::ffi::c_void,
        symbol: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void {
        let symbol = unsafe { std::ffi::CStr::from_ptr(symbol) };
        self.symbols.borrow_mut().push(symbol.to_bytes().to_vec());
        self.symbol_result
    }

    unsafe fn error(&self) -> *const std::ffi::c_char {
        self.errors
            .borrow_mut()
            .remove(0)
            .map_or(std::ptr::null(), |error| error.into_raw())
    }
}

unsafe extern "C" fn non_null_sciter_api() -> *const crate::sciter::bindings::ISciterAPI {
    std::ptr::NonNull::<crate::sciter::bindings::ISciterAPI>::dangling().as_ptr()
}

unsafe extern "C" fn null_sciter_api() -> *const crate::sciter::bindings::ISciterAPI {
    std::ptr::null()
}

fn successful_dynamic_loader() -> FixtureDynamicLoader {
    FixtureDynamicLoader {
        open_result: std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr(),
        symbol_result: non_null_sciter_api as *const () as *mut std::ffi::c_void,
        errors: RefCell::new(vec![None, None]),
        ..FixtureDynamicLoader::default()
    }
}

#[test]
fn sciter_runtime_rejects_invalid_nonabsolute_noncanonical_and_mismatched_paths_before_ffi() {
    use std::os::unix::ffi::OsStrExt;

    let directory = TestDirectory::new("sciter-runtime-paths");
    let runtime = directory.path.join("libsciter.dylib");
    let alternate = directory.path.join("alternate.dylib");
    fs::write(&runtime, b"runtime").unwrap();
    fs::write(&alternate, b"alternate").unwrap();
    let runtime = runtime.canonicalize().unwrap();
    let alternate = alternate.canonicalize().unwrap();

    for (path, expected) in [
        (PathBuf::from("libsciter.dylib"), "nonabsolute"),
        (directory.path.join("./libsciter.dylib"), "noncanonical"),
    ] {
        let loader = successful_dynamic_loader();
        let error = unsafe { SciterRuntime::load_with(&path, &runtime, &loader) }.unwrap_err();
        assert!(matches!(
            (expected, error),
            ("nonabsolute", RuntimeLoadError::NonAbsolutePath { .. })
                | ("noncanonical", RuntimeLoadError::NonCanonicalPath { .. })
        ));
        assert!(loader.opens.borrow().is_empty());
    }

    let invalid = PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/lib\0sciter.dylib"));
    let loader = successful_dynamic_loader();
    assert!(matches!(
        unsafe { SciterRuntime::load_with(&invalid, &runtime, &loader) },
        Err(RuntimeLoadError::InvalidPath { .. })
    ));
    assert!(loader.opens.borrow().is_empty());

    let loader = successful_dynamic_loader();
    assert!(matches!(
        unsafe { SciterRuntime::load_with(&alternate, &runtime, &loader) },
        Err(RuntimeLoadError::PathMismatch { .. })
    ));
    assert!(loader.opens.borrow().is_empty());
}

#[test]
fn sciter_runtime_loads_only_the_exact_canonical_manifest_path_and_export() {
    let directory = TestDirectory::new("sciter-runtime-exact-load");
    let runtime = directory.path.join("libsciter.dylib");
    fs::write(&runtime, b"runtime").unwrap();
    let runtime = runtime.canonicalize().unwrap();
    let loader = successful_dynamic_loader();

    let loaded = unsafe { SciterRuntime::load_with(&runtime, &runtime, &loader) }.unwrap();

    assert_eq!(
        loader.opens.borrow().as_slice(),
        &[(
            runtime.as_os_str().as_encoded_bytes().to_vec(),
            RTLD_NOW | RTLD_LOCAL
        )]
    );
    assert_eq!(loader.symbols.borrow().as_slice(), &[b"SciterAPI".to_vec()]);
    assert_eq!(
        loaded.api_table(),
        std::ptr::NonNull::<crate::sciter::bindings::ISciterAPI>::dangling()
    );
}

#[test]
fn sciter_runtime_returns_copied_load_symbol_and_null_table_failures() {
    let directory = TestDirectory::new("sciter-runtime-failures");
    let runtime = directory.path.join("libsciter.dylib");
    fs::write(&runtime, b"runtime").unwrap();
    let runtime = runtime.canonicalize().unwrap();

    let load_failure = FixtureDynamicLoader {
        errors: RefCell::new(vec![Some(
            std::ffi::CString::new("pinned load failed").unwrap(),
        )]),
        ..FixtureDynamicLoader::default()
    };
    assert!(matches!(
        unsafe { SciterRuntime::load_with(&runtime, &runtime, &load_failure) },
        Err(RuntimeLoadError::LoadFailure { diagnostic, .. }) if diagnostic == "pinned load failed"
    ));

    let symbol_failure = FixtureDynamicLoader {
        open_result: std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr(),
        errors: RefCell::new(vec![
            None,
            Some(std::ffi::CString::new("export absent").unwrap()),
        ]),
        ..FixtureDynamicLoader::default()
    };
    assert!(matches!(
        unsafe { SciterRuntime::load_with(&runtime, &runtime, &symbol_failure) },
        Err(RuntimeLoadError::SymbolResolutionFailure { diagnostic, .. }) if diagnostic == "export absent"
    ));

    let null_table = FixtureDynamicLoader {
        open_result: std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr(),
        symbol_result: null_sciter_api as *const () as *mut std::ffi::c_void,
        errors: RefCell::new(vec![None, None]),
        ..FixtureDynamicLoader::default()
    };
    assert!(matches!(
        unsafe { SciterRuntime::load_with(&runtime, &runtime, &null_table) },
        Err(RuntimeLoadError::NullApiTable)
    ));
}

#[test]
#[ignore = "requires the pinned Sciter dylib on a native arm64 macOS host"]
fn pinned_sciter_runtime_load_smoke() {
    let repository_root = toolkit_dir().join("../..").canonicalize().unwrap();
    let manifest = fixed_manifest();
    let runtime = repository_root
        .join(manifest.workspace_relative_path())
        .canonicalize()
        .unwrap();
    let hash = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(&runtime)
        .output()
        .unwrap();
    assert!(hash.status.success());
    assert_eq!(
        String::from_utf8(hash.stdout)
            .unwrap()
            .split_whitespace()
            .next(),
        Some(manifest.sha256())
    );

    let loaded = unsafe { SciterRuntime::load_absolute(&runtime, &runtime) }.unwrap();
    let _api_table = loaded.api_table();
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mdluma-build-entry-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn toolkit_dir() -> PathBuf {
    Path::new(file!())
        .canonicalize()
        .expect("canonicalize tests.rs")
        .parent()
        .expect("tests.rs has a parent")
        .to_path_buf()
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable fixture");
    let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

fn fake_path(test_dir: &TestDirectory, host_arch: &str, rust_arch: &str) -> PathBuf {
    let bin_dir = test_dir.path.join("bin");
    fs::create_dir(&bin_dir).expect("create fake bin directory");
    write_executable(
        &bin_dir.join("uname"),
        &format!(
            "#!/bin/sh\ncase \"$1\" in\n  -s) echo Darwin ;;\n  -m) echo {host_arch} ;;\n  *) exit 2 ;;\nesac\n"
        ),
    );
    write_executable(
        &bin_dir.join("rustc"),
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$BUILD_ENTRY_RUSTC_LOG"
if [ "$1" = "--print" ] && [ "$2" = "cfg" ]; then
  echo 'target_arch="{rust_arch}"'
  exit 0
fi
output=
previous=
for argument in "$@"; do
  if [ "$previous" = "-o" ]; then output=$argument; fi
  previous=$argument
done
[ -n "$output" ] || exit 3
cat > "$output" <<'PROGRAM'
#!/bin/sh
printf 'executed\n' > "$BUILD_ENTRY_EXECUTION_MARKER"
PROGRAM
chmod +x "$output"
"#,
        ),
    );
    bin_dir
}

fn invoke(mode: &str, test_dir: &TestDirectory, host_arch: &str, rust_arch: &str) -> Output {
    let rustc_log = test_dir.path.join("rustc.log");
    let marker = test_dir.path.join("executed");
    let bin_dir = fake_path(test_dir, host_arch, rust_arch);
    let inherited_path = std::env::var_os("PATH").expect("PATH is set");
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited_path));
    let path = std::env::join_paths(paths).expect("compose PATH");

    Command::new(toolkit_dir().join("run.sh"))
        .arg(mode)
        .current_dir(&test_dir.path)
        .env("PATH", path)
        .env("TMPDIR", &test_dir.path)
        .env("BUILD_ENTRY_RUSTC_LOG", &rustc_log)
        .env("BUILD_ENTRY_EXECUTION_MARKER", &marker)
        .output()
        .expect("invoke build entry")
}

#[test]
fn run_mode_builds_from_the_repository_root_and_executes_temporary_binary() {
    let test_dir = TestDirectory::new("run");
    let output = invoke("run", &test_dir, "arm64", "aarch64");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(test_dir.path.join("executed")).unwrap(),
        "executed\n"
    );
    let rustc_log = fs::read_to_string(test_dir.path.join("rustc.log")).unwrap();
    assert!(rustc_log.contains(&toolkit_dir().join("main.rs").display().to_string()));
    assert!(rustc_log.contains("--edition=2021"));
    assert!(!rustc_log.contains("--test"));
    let output_path = rustc_log
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|arguments| (arguments[0] == "-o").then_some(Path::new(arguments[1])))
        .expect("rustc output path");
    assert!(
        !output_path.exists(),
        "temporary build directory must be removed"
    );
    assert!(!toolkit_dir().join("target").exists());
}

#[test]
fn test_mode_builds_and_executes_a_native_test_binary() {
    let test_dir = TestDirectory::new("test");
    let output = invoke("test", &test_dir, "arm64", "aarch64");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(test_dir.path.join("executed").is_file());
    let rustc_log = fs::read_to_string(test_dir.path.join("rustc.log")).unwrap();
    assert!(rustc_log.contains("--test"));
}

#[test]
fn non_arm64_host_stops_before_compilation_or_execution() {
    let test_dir = TestDirectory::new("host-arch");
    let output = invoke("run", &test_dir, "x86_64", "aarch64");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("arm64"));
    assert!(!test_dir.path.join("rustc.log").exists());
    assert!(!test_dir.path.join("executed").exists());
}

#[test]
fn non_arm64_rust_target_stops_before_binary_build_or_execution() {
    let test_dir = TestDirectory::new("rust-arch");
    let output = invoke("run", &test_dir, "arm64", "x86_64");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("aarch64"));
    assert_eq!(
        fs::read_to_string(test_dir.path.join("rustc.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(!test_dir.path.join("executed").exists());
}

#[test]
fn invalid_mode_is_rejected_before_prerequisite_checks() {
    let test_dir = TestDirectory::new("invalid-mode");
    let output = invoke("unsupported", &test_dir, "arm64", "aarch64");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
    assert!(!test_dir.path.join("rustc.log").exists());
}

#[test]
fn missing_rustc_stops_with_a_diagnostic_before_binary_execution() {
    let test_dir = TestDirectory::new("missing-rustc");
    let bin_dir = fake_path(&test_dir, "arm64", "aarch64");
    let inherited_path = std::env::var_os("PATH").expect("PATH is set");
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited_path));

    let output = Command::new(toolkit_dir().join("run.sh"))
        .arg("run")
        .current_dir(&test_dir.path)
        .env("PATH", std::env::join_paths(paths).expect("compose PATH"))
        .env("RUSTC", "missing-rustc")
        .env("BUILD_ENTRY_RUSTC_LOG", test_dir.path.join("rustc.log"))
        .env(
            "BUILD_ENTRY_EXECUTION_MARKER",
            test_dir.path.join("executed"),
        )
        .output()
        .expect("invoke build entry");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("native rustc is required"));
    assert!(!test_dir.path.join("executed").exists());
}

#[test]
fn build_entry_does_not_preflight_artifact_metadata_commands() {
    let script = fs::read_to_string(toolkit_dir().join("run.sh")).expect("read run.sh");

    for later_phase_command in ["shasum", "lipo", "otool", "codesign", "sw_vers", "sysctl"] {
        assert!(
            !script.contains(later_phase_command),
            "{later_phase_command} belongs to ArtifactProbe or EvidenceRunner"
        );
    }
}

#[test]
fn criterion_catalogs_contain_exactly_78_known_and_67_source_ids() {
    let mut expected = Vec::new();
    for (requirement, last_criterion) in [
        (1, 7),
        (2, 8),
        (3, 9),
        (4, 10),
        (5, 10),
        (6, 11),
        (7, 12),
        (8, 11),
    ] {
        expected.extend(
            (1..=last_criterion).map(|criterion| CriterionId::from_parts(requirement, criterion)),
        );
    }

    assert_eq!(ALL_CRITERIA, expected);
    assert_eq!(SOURCE_CRITERIA, &expected[..67]);
}

#[test]
fn criterion_result_validation_rejects_missing_duplicate_unknown_and_unauthorized_na() {
    let satisfied = |id| CriterionResult::new(id, CriterionStatus::Satisfied, "ok", vec![]);
    let complete: Vec<_> = SOURCE_CRITERIA.iter().copied().map(satisfied).collect();
    validate_criterion_results(&complete, SOURCE_CRITERIA).unwrap();

    let mut missing = complete.clone();
    missing.pop();
    assert!(matches!(
        validate_criterion_results(&missing, SOURCE_CRITERIA),
        Err(EvidenceError::MissingCriterion(_))
    ));

    let mut duplicate = complete.clone();
    duplicate.push(complete[0].clone());
    assert!(matches!(
        validate_criterion_results(&duplicate, SOURCE_CRITERIA),
        Err(EvidenceError::DuplicateCriterion(_))
    ));

    let unknown = CriterionId::from_parts(9, 1);
    let mut with_unknown = complete.clone();
    with_unknown[0] = satisfied(unknown);
    assert!(matches!(
        validate_criterion_results(&with_unknown, SOURCE_CRITERIA),
        Err(EvidenceError::UnknownCriterion(id)) if id == unknown
    ));
}

#[test]
fn not_applicable_is_authorized_for_exactly_the_explicit_conditional_ids() {
    let expected = [
        (1, 5),
        (1, 6),
        (1, 7),
        (2, 1),
        (2, 2),
        (2, 3),
        (2, 4),
        (3, 3),
        (3, 4),
        (3, 5),
        (3, 6),
        (3, 7),
        (3, 8),
        (3, 9),
        (4, 3),
        (4, 4),
        (4, 5),
        (4, 6),
        (4, 7),
        (4, 8),
        (4, 9),
        (5, 2),
        (5, 3),
        (5, 6),
        (5, 7),
        (5, 8),
        (5, 9),
        (6, 7),
        (6, 8),
        (6, 9),
        (6, 10),
        (6, 11),
        (7, 10),
        (7, 11),
        (7, 12),
        (8, 2),
        (8, 3),
        (8, 4),
        (8, 5),
        (8, 6),
        (8, 7),
        (8, 8),
        (8, 9),
        (8, 10),
    ]
    .map(|(requirement, criterion)| CriterionId::from_parts(requirement, criterion));

    let actual: Vec<_> = ALL_CRITERIA
        .iter()
        .copied()
        .filter(|id| id.permits_not_applicable())
        .collect();
    assert_eq!(actual, expected);

    for id in ALL_CRITERIA.iter().copied() {
        let result = CriterionResult::try_new(
            id,
            CriterionStatus::NotApplicable,
            "trigger did not apply",
            vec![],
        );
        assert_eq!(result.is_ok(), expected.contains(&id), "criterion {id}");
    }
}

#[test]
fn gate_catalog_and_mappings_cover_exactly_eight_required_gates() {
    let expected_gates = [
        GateId::Artifact,
        GateId::Platform,
        GateId::Api,
        GateId::Abi,
        GateId::Popup,
        GateId::Window,
        GateId::License,
        GateId::Record,
    ];
    assert_eq!(ALL_GATES, expected_gates);

    let mappings: &[(GateId, &[(u8, u8)])] = &[
        (
            GateId::Artifact,
            &[(1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6), (1, 7)],
        ),
        (
            GateId::Platform,
            &[
                (2, 1),
                (2, 2),
                (2, 3),
                (2, 4),
                (2, 5),
                (2, 6),
                (2, 7),
                (2, 8),
            ],
        ),
        (
            GateId::Api,
            &[
                (3, 1),
                (3, 2),
                (3, 3),
                (3, 4),
                (3, 5),
                (3, 6),
                (3, 7),
                (3, 8),
                (3, 9),
            ],
        ),
        (
            GateId::Abi,
            &[
                (4, 1),
                (4, 2),
                (4, 3),
                (4, 4),
                (4, 5),
                (4, 6),
                (4, 7),
                (4, 8),
                (4, 9),
                (4, 10),
            ],
        ),
        (
            GateId::Popup,
            &[(5, 1), (5, 2), (5, 4), (5, 6), (5, 8), (5, 9), (5, 10)],
        ),
        (
            GateId::Window,
            &[(5, 1), (5, 3), (5, 5), (5, 7), (5, 8), (5, 9), (5, 10)],
        ),
        (
            GateId::License,
            &[
                (6, 1),
                (6, 2),
                (6, 3),
                (6, 4),
                (6, 5),
                (6, 6),
                (6, 7),
                (6, 8),
                (6, 9),
                (6, 10),
                (6, 11),
            ],
        ),
        (
            GateId::Record,
            &[
                (7, 1),
                (7, 2),
                (7, 3),
                (7, 4),
                (7, 5),
                (7, 6),
                (7, 7),
                (7, 8),
                (7, 9),
                (7, 10),
                (7, 11),
                (7, 12),
            ],
        ),
    ];

    for (gate, expected) in mappings {
        let expected: Vec<_> = expected
            .iter()
            .map(|&(requirement, criterion)| CriterionId::from_parts(requirement, criterion))
            .collect();
        assert_eq!(gate.criteria(), expected, "gate {gate:?}");
    }
}

#[test]
fn harness_events_reject_invalid_cycle_and_kind_phase_combinations() {
    HarnessEvent::new(1, CycleKind::Popup, 1, CyclePhase::Started).unwrap();
    HarnessEvent::new(1, CycleKind::Popup, 100, CyclePhase::Closed).unwrap();
    HarnessEvent::new(1, CycleKind::Window, 42, CyclePhase::Destroyed).unwrap();

    assert!(matches!(
        HarnessEvent::new(1, CycleKind::Popup, 0, CyclePhase::Started),
        Err(EvidenceError::InvalidCycle(0))
    ));
    assert!(matches!(
        HarnessEvent::new(1, CycleKind::Window, 101, CyclePhase::Started),
        Err(EvidenceError::InvalidCycle(101))
    ));
    assert!(matches!(
        HarnessEvent::new(1, CycleKind::Popup, 1, CyclePhase::Created),
        Err(EvidenceError::InvalidCyclePhase {
            kind: CycleKind::Popup,
            phase: CyclePhase::Created,
        })
    ));
    assert!(matches!(
        HarnessEvent::new(1, CycleKind::Window, 1, CyclePhase::Shown),
        Err(EvidenceError::InvalidCyclePhase {
            kind: CycleKind::Window,
            phase: CyclePhase::Shown,
        })
    ));
}

#[test]
fn run_identity_is_a_nonempty_safe_path_component() {
    assert_eq!(
        RunId::new("20260727T010203Z-a1b2").unwrap().as_str(),
        "20260727T010203Z-a1b2"
    );
    for invalid in ["", ".", "..", "run/child", "run\nchild"] {
        assert!(matches!(
            RunId::new(invalid),
            Err(EvidenceError::InvalidRunId(_))
        ));
    }
}

#[test]
fn shared_status_and_error_types_retain_typed_state() {
    assert_ne!(CriterionStatus::Unsatisfied, CriterionStatus::NotRun);
    assert_ne!(GateStatus::Fail, GateStatus::NotRun);
    assert_ne!(DecisionState::Pending, DecisionState::NoGo);

    let gate = GateResult::new(
        GateId::Popup,
        GateStatus::Pass,
        GateId::Popup.criteria().to_vec(),
        "popup complete",
    );
    assert_eq!(gate.id(), GateId::Popup);
    assert_eq!(gate.status(), GateStatus::Pass);
    assert_eq!(gate.criteria(), GateId::Popup.criteria());
    assert_eq!(gate.summary(), "popup complete");

    let timeout = EvidenceError::Timeout {
        kind: CycleKind::Window,
        cycle: 17,
    };
    assert!(matches!(
        timeout,
        EvidenceError::Timeout {
            kind: CycleKind::Window,
            cycle: 17,
        }
    ));
}

#[test]
fn initial_artifact_manifest_parses_to_the_fixed_baseline() {
    let manifest_path = toolkit_dir()
        .join("../..")
        .join(".kiro/specs/macos-sciter-runtime-evidence/evidence/artifact-manifest.txt");
    let input = fs::read_to_string(manifest_path).expect("read initial artifact manifest");
    let manifest = parse_artifact_manifest(&input).expect("parse initial artifact manifest");

    assert_eq!(manifest.schema_version(), 1);
    assert_eq!(
        manifest.repository(),
        "https://gitlab.com/sciter-engine/sciter-js-sdk"
    );
    assert_eq!(
        manifest.commit(),
        "e31ec0f726bdbe5d0402ad647f3b34feef84654e"
    );
    assert_eq!(
        manifest.sdk_relative_path(),
        Path::new("bin/macosx/libsciter.dylib")
    );
    assert_eq!(
        manifest.workspace_relative_path(),
        Path::new("vendor/sciter-js-sdk-main/bin/macosx/libsciter.dylib")
    );
    assert_eq!(
        manifest.sha256(),
        "be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b"
    );
    assert_eq!(manifest.engine_version(), [6, 0, 3, 18]);
    assert_eq!(manifest.api_version(), 10);
    assert_eq!(
        manifest.version_header_path(),
        Path::new("vendor/sciter-js-sdk-main/include/sciter-version.h")
    );
    assert_eq!(
        manifest.api_header_path(),
        Path::new("vendor/sciter-js-sdk-main/include/sciter-x-api.h")
    );
    assert_eq!(
        manifest.version_header_source(),
        "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-version.h"
    );
    assert_eq!(
        manifest.api_header_source(),
        "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-x-api.h"
    );
}

#[test]
fn artifact_manifest_rejects_unknown_duplicate_missing_and_malformed_lines() {
    let unknown = format!("{ARTIFACT_MANIFEST}extra=value\n");
    let duplicate = format!("{ARTIFACT_MANIFEST}api_version=10\n");
    let missing = ARTIFACT_MANIFEST.replace("api_version=10\n", "");

    for input in [
        unknown,
        duplicate,
        missing,
        format!("{ARTIFACT_MANIFEST}broken\n"),
    ] {
        assert!(matches!(
            parse_artifact_manifest(&input),
            Err(EvidenceError::InvalidManifest(_))
        ));
    }
}

#[test]
fn artifact_manifest_rejects_malformed_hash_versions_numbers_and_paths() {
    let invalid_inputs = [
        ARTIFACT_MANIFEST.replace(
            "sha256=be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b",
            "sha256=not-a-sha256",
        ),
        ARTIFACT_MANIFEST.replace("engine_version=6.0.3.18", "engine_version=6.0.3"),
        ARTIFACT_MANIFEST.replace("api_version=10", "api_version=ten"),
        ARTIFACT_MANIFEST.replace("schema_version=1", "schema_version=one"),
        ARTIFACT_MANIFEST.replace(
            "sdk_relative_path=bin/macosx/libsciter.dylib",
            "sdk_relative_path=/bin/macosx/libsciter.dylib",
        ),
        ARTIFACT_MANIFEST.replace(
            "workspace_relative_path=vendor/sciter-js-sdk-main/bin/macosx/libsciter.dylib",
            "workspace_relative_path=vendor/../libsciter.dylib",
        ),
        ARTIFACT_MANIFEST.replace(
            "version_header_path=vendor/sciter-js-sdk-main/include/sciter-version.h",
            "version_header_path=",
        ),
        ARTIFACT_MANIFEST.replace(
            "api_header_path=vendor/sciter-js-sdk-main/include/sciter-x-api.h",
            "api_header_path=C:\\sciter-x-api.h",
        ),
    ];

    for input in invalid_inputs {
        assert!(matches!(
            parse_artifact_manifest(&input),
            Err(EvidenceError::InvalidManifest(_))
        ));
    }
}

#[test]
fn artifact_manifest_rejects_well_formed_values_outside_the_fixed_baseline() {
    let mismatches = [
        ARTIFACT_MANIFEST.replace("schema_version=1", "schema_version=2"),
        ARTIFACT_MANIFEST.replace(
            "repository=https://gitlab.com/sciter-engine/sciter-js-sdk",
            "repository=https://example.com/sciter-js-sdk",
        ),
        ARTIFACT_MANIFEST.replace(
            "commit=e31ec0f726bdbe5d0402ad647f3b34feef84654e",
            "commit=031ec0f726bdbe5d0402ad647f3b34feef84654e",
        ),
        ARTIFACT_MANIFEST.replace(
            "sha256=be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b",
            "sha256=0e5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b",
        ),
        ARTIFACT_MANIFEST.replace("engine_version=6.0.3.18", "engine_version=6.0.3.19"),
        ARTIFACT_MANIFEST.replace("api_version=10", "api_version=11"),
        ARTIFACT_MANIFEST.replace(
            "version_header_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-version.h",
            "version_header_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/031ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-version.h",
        ),
    ];

    for input in mismatches {
        assert!(matches!(
            parse_artifact_manifest(&input),
            Err(EvidenceError::InvalidManifest(_))
        ));
    }
}

#[test]
fn artifact_manifest_rejects_noncanonical_fixed_value_representations() {
    let noncanonical_inputs = [
        ARTIFACT_MANIFEST.replace("schema_version=1", "schema_version=01"),
        ARTIFACT_MANIFEST.replace("api_version=10", "api_version=010"),
        ARTIFACT_MANIFEST.replace("engine_version=6.0.3.18", "engine_version=6.00.3.18"),
        ARTIFACT_MANIFEST.replace(
            "sdk_relative_path=bin/macosx/libsciter.dylib",
            "sdk_relative_path=bin//macosx/libsciter.dylib",
        ),
        ARTIFACT_MANIFEST.replace(
            "workspace_relative_path=vendor/sciter-js-sdk-main/bin/macosx/libsciter.dylib",
            "workspace_relative_path=vendor/sciter-js-sdk-main//bin/macosx/libsciter.dylib",
        ),
        ARTIFACT_MANIFEST.replace(
            "version_header_path=vendor/sciter-js-sdk-main/include/sciter-version.h",
            "version_header_path=vendor/sciter-js-sdk-main//include/sciter-version.h",
        ),
        ARTIFACT_MANIFEST.replace(
            "api_header_path=vendor/sciter-js-sdk-main/include/sciter-x-api.h",
            "api_header_path=vendor/sciter-js-sdk-main//include/sciter-x-api.h",
        ),
    ];

    for input in noncanonical_inputs {
        assert!(matches!(
            parse_artifact_manifest(&input),
            Err(EvidenceError::InvalidManifest(_))
        ));
    }
}

#[test]
fn initial_license_evidence_parses_without_guessing_provider_permissions() {
    let evidence_path = toolkit_dir()
        .join("../..")
        .join(".kiro/specs/macos-sciter-runtime-evidence/evidence/license-evidence.txt");
    let input = fs::read_to_string(evidence_path).expect("read initial license evidence");
    let evidence = parse_license_evidence(&input).expect("parse initial license evidence");

    assert_eq!(evidence.schema_version(), 1);
    assert_eq!(evidence.redistribution(), PermissionStatus::Unresolved);
    assert_eq!(evidence.resigning(), PermissionStatus::Unresolved);
    assert_eq!(
        evidence.license_source(),
        "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/LICENSE"
    );
    assert_eq!(
        evidence.eula_source(),
        "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md"
    );
    assert_eq!(
        evidence.permission_source(),
        "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md"
    );
    assert_eq!(
        evidence.required_about_text(),
        "This application uses Sciter Engine (http://sciter.com/), copyright Terra Informatica Software, Inc."
    );
    assert_eq!(
        evidence.required_distribution_files(),
        ["LICENSE".to_owned(), "SCITER-ENGINE-EULA.md".to_owned()]
    );
}

#[test]
fn license_evidence_accepts_all_typed_statuses_and_authoritative_permission_sources() {
    for (text, status) in [
        ("permitted", PermissionStatus::Permitted),
        ("prohibited", PermissionStatus::Prohibited),
        ("unresolved", PermissionStatus::Unresolved),
    ] {
        let input = LICENSE_EVIDENCE
            .replace(
                "redistribution=unresolved",
                &format!("redistribution={text}"),
            )
            .replace("resigning=unresolved", &format!("resigning={text}"))
            .replace(
                "permission_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md",
                "permission_source=https://sciter.com/authoritative-permission",
            );
        let evidence = parse_license_evidence(&input).unwrap();
        assert_eq!(evidence.redistribution(), status);
        assert_eq!(evidence.resigning(), status);
        assert_eq!(
            evidence.permission_source(),
            "https://sciter.com/authoritative-permission"
        );
    }
}

#[test]
fn license_evidence_rejects_unknown_duplicate_missing_empty_and_invalid_status_fields() {
    let invalid_inputs = [
        format!("{LICENSE_EVIDENCE}extra=value\n"),
        format!("{LICENSE_EVIDENCE}resigning=unresolved\n"),
        LICENSE_EVIDENCE.replace(
            "permission_source=https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md\n",
            "",
        ),
        LICENSE_EVIDENCE.replace("required_about_text=This", "required_about_text="),
        LICENSE_EVIDENCE.replace("redistribution=unresolved", "redistribution=allowed"),
        LICENSE_EVIDENCE.replace("schema_version=1", "schema_version=2"),
    ];

    for input in invalid_inputs {
        assert!(matches!(
            parse_license_evidence(&input),
            Err(EvidenceError::InvalidManifest(_))
        ));
    }
}

#[test]
fn license_evidence_rejects_leading_zero_schema_version() {
    let input = LICENSE_EVIDENCE.replace("schema_version=1", "schema_version=01");

    assert!(matches!(
        parse_license_evidence(&input),
        Err(EvidenceError::InvalidManifest(_))
    ));
}

#[test]
fn license_evidence_rejects_placeholder_and_invalid_permission_sources() {
    for source in [
        "unresolved",
        "<official public document or written response>",
        "https://example.com/permission",
        "https://sciter.com/docs//permission",
        "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/../LICENSE",
        "../private/provider-response.txt",
    ] {
        let input = LICENSE_EVIDENCE.replace(
            "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md\nrequired_about_text",
            &format!("{source}\nrequired_about_text"),
        );
        assert!(
            matches!(
                parse_license_evidence(&input),
                Err(EvidenceError::InvalidManifest(_))
            ),
            "source: {source}"
        );
    }
}

#[test]
fn license_evidence_rejects_noncanonical_about_attribution() {
    let input = LICENSE_EVIDENCE.replace(
        "This application uses Sciter Engine (http://sciter.com/), copyright Terra Informatica Software, Inc.",
        "This app uses Sciter.",
    );

    assert!(matches!(
        parse_license_evidence(&input),
        Err(EvidenceError::InvalidManifest(_))
    ));
}

#[test]
fn license_evidence_requires_the_exact_distribution_file_set_once() {
    for files in [
        "LICENSE",
        "LICENSE,NOTICE",
        "SCITER-ENGINE-EULA.md,LICENSE",
        "LICENSE,SCITER-ENGINE-EULA.md,LICENSE",
    ] {
        let input = LICENSE_EVIDENCE.replace(
            "required_distribution_files=LICENSE,SCITER-ENGINE-EULA.md",
            &format!("required_distribution_files={files}"),
        );
        assert!(
            matches!(
                parse_license_evidence(&input),
                Err(EvidenceError::InvalidManifest(_))
            ),
            "files: {files}"
        );
    }
}

#[test]
fn license_evidence_rejects_noncanonical_distribution_paths() {
    for files in [
        "./LICENSE,SCITER-ENGINE-EULA.md",
        "licenses//LICENSE,SCITER-ENGINE-EULA.md",
        "LICENSE,docs//SCITER-ENGINE-EULA.md",
        "LICENSE,SCITER-ENGINE-EULA.md/.",
    ] {
        let input = LICENSE_EVIDENCE.replace(
            "required_distribution_files=LICENSE,SCITER-ENGINE-EULA.md",
            &format!("required_distribution_files={files}"),
        );
        assert!(
            matches!(
                parse_license_evidence(&input),
                Err(EvidenceError::InvalidManifest(_))
            ),
            "files: {files}"
        );
    }
}

#[test]
fn license_evidence_rejects_wrong_revision_sources_and_unsafe_distribution_paths() {
    let invalid_inputs = [
        LICENSE_EVIDENCE.replace(
            "e31ec0f726bdbe5d0402ad647f3b34feef84654e/LICENSE",
            "031ec0f726bdbe5d0402ad647f3b34feef84654e/LICENSE",
        ),
        LICENSE_EVIDENCE.replace(
            "e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md",
            "031ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md",
        ),
        LICENSE_EVIDENCE.replace(
            "required_distribution_files=LICENSE,SCITER-ENGINE-EULA.md",
            "required_distribution_files=LICENSE,../SCITER-ENGINE-EULA.md",
        ),
        LICENSE_EVIDENCE.replace(
            "required_distribution_files=LICENSE,SCITER-ENGINE-EULA.md",
            "required_distribution_files=/LICENSE,SCITER-ENGINE-EULA.md",
        ),
        LICENSE_EVIDENCE.replace(
            "required_distribution_files=LICENSE,SCITER-ENGINE-EULA.md",
            "required_distribution_files=LICENSE,C:\\SCITER-ENGINE-EULA.md",
        ),
        LICENSE_EVIDENCE.replace(
            "required_distribution_files=LICENSE,SCITER-ENGINE-EULA.md",
            "required_distribution_files=LICENSE,,SCITER-ENGINE-EULA.md",
        ),
    ];

    for input in invalid_inputs {
        assert!(matches!(
            parse_license_evidence(&input),
            Err(EvidenceError::InvalidManifest(_))
        ));
    }
}

#[test]
fn artifact_probe_collects_typed_metadata_and_lossless_command_artifacts() {
    let (repository, runtime) = artifact_fixture("artifact-success");
    let runner = FixtureCommandRunner::successful();

    let bundle = probe_artifact_with_runner(&fixed_manifest(), &repository.path, &runtime, &runner);

    assert_eq!(bundle.actual_sha256.as_deref(), Some(EXPECTED_SHA256));
    assert!(bundle.hash_matches);
    assert_eq!(
        bundle.provenance_repository,
        "https://gitlab.com/sciter-engine/sciter-js-sdk"
    );
    assert_eq!(
        bundle.provenance_commit,
        "e31ec0f726bdbe5d0402ad647f3b34feef84654e"
    );
    assert_eq!(
        bundle.provenance_sdk_path,
        Path::new("bin/macosx/libsciter.dylib")
    );
    assert_eq!(
        bundle.workspace_runtime_path.as_deref(),
        bundle.runtime_path.as_deref()
    );
    assert_ne!(
        bundle.provenance_sdk_path,
        fixed_manifest().workspace_relative_path
    );
    assert_eq!(bundle.architectures, ["x86_64", "arm64"]);
    assert_eq!(
        bundle.minimum_macos_versions,
        [
            ("x86_64".to_owned(), "11.5".to_owned()),
            ("arm64".to_owned(), "11.5".to_owned())
        ]
    );
    assert_eq!(bundle.dependencies, ["/usr/lib/libSystem.B.dylib"]);
    assert_eq!(
        bundle.install_name.as_deref(),
        Some("/usr/local/lib/libsciter.dylib")
    );
    assert_eq!(bundle.codesign_state, Some(CodeSigningState::AdHoc));
    assert_eq!(bundle.raw_artifacts.len(), 18);
    assert!(bundle.raw_artifacts.iter().any(|artifact| {
        artifact.relative_path == Path::new("metadata/codesign.stderr")
            && artifact.bytes
                == b"Executable=/tmp/runtime\nSignature=adhoc\nTeamIdentifier=not set\n"
    }));
    assert_eq!(
        bundle
            .gates
            .iter()
            .map(GateResult::status)
            .collect::<Vec<_>>(),
        [GateStatus::Pass, GateStatus::NotRun]
    );

    let expected_raw = [
        (
            "shasum",
            format!("{EXPECTED_SHA256}  runtime\n").into_bytes(),
            b"shasum stderr\n".to_vec(),
        ),
        ("lipo", b"x86_64 arm64\n".to_vec(), b"lipo stderr\n".to_vec()),
        (
            "otool-load",
            b"runtime (architecture x86_64):\n      cmd LC_BUILD_VERSION\n    minos 11.5\n     tool 3\n  version 1266.8\nruntime (architecture arm64):\n      cmd LC_BUILD_VERSION\n    minos 11.5\n     tool 3\n  version 1266.8\n".to_vec(),
            b"otool load stderr\n".to_vec(),
        ),
        (
            "otool-dependencies",
            b"runtime (architecture x86_64):\n\t/usr/local/lib/libsciter.dylib (compatibility version 1.0.0, current version 1.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\nruntime (architecture arm64):\n\t/usr/local/lib/libsciter.dylib (compatibility version 1.0.0, current version 1.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\n".to_vec(),
            b"otool dependencies stderr\n".to_vec(),
        ),
        (
            "otool-install-name",
            b"runtime (architecture x86_64):\n/usr/local/lib/libsciter.dylib\nruntime (architecture arm64):\n/usr/local/lib/libsciter.dylib\n".to_vec(),
            b"otool install name stderr\n".to_vec(),
        ),
        (
            "codesign",
            b"codesign stdout\n".to_vec(),
            b"Executable=/tmp/runtime\nSignature=adhoc\nTeamIdentifier=not set\n".to_vec(),
        ),
    ];
    for (name, stdout, stderr) in expected_raw {
        for (suffix, expected) in [
            ("stdout", stdout.as_slice()),
            ("stderr", stderr.as_slice()),
            ("status", b"Exited(0)\n".as_slice()),
        ] {
            let path = PathBuf::from(format!("metadata/{name}.{suffix}"));
            let artifact = bundle
                .raw_artifacts
                .iter()
                .find(|artifact| artifact.relative_path == path)
                .unwrap();
            assert_eq!(artifact.bytes, expected, "{}", path.display());
        }
    }

    let canonical_runtime = runtime.canonicalize().unwrap().into_os_string();
    assert_eq!(
        runner.calls(),
        [
            (
                "shasum".to_owned(),
                vec!["-a".into(), "256".into(), canonical_runtime.clone()]
            ),
            (
                "lipo".to_owned(),
                vec!["-archs".into(), canonical_runtime.clone()]
            ),
            (
                "otool".to_owned(),
                vec!["-l".into(), canonical_runtime.clone()]
            ),
            (
                "otool".to_owned(),
                vec!["-L".into(), canonical_runtime.clone()]
            ),
            (
                "otool".to_owned(),
                vec!["-D".into(), canonical_runtime.clone()]
            ),
            (
                "codesign".to_owned(),
                vec!["-dv".into(), "--verbose=4".into(), canonical_runtime]
            ),
        ]
    );
}

#[test]
fn artifact_probe_hash_mismatch_and_arm64_absence_fail_their_typed_gates() {
    let (repository, runtime) = artifact_fixture("artifact-hash-mismatch");
    let runner = FixtureCommandRunner::successful();
    runner.captures.borrow_mut()[0].stdout = format!("{}  runtime\n", "0".repeat(64)).into_bytes();

    let bundle = probe_artifact_with_runner(&fixed_manifest(), &repository.path, &runtime, &runner);

    assert!(!bundle.hash_matches);
    assert_eq!(bundle.gates[0].status(), GateStatus::Fail);
    assert_eq!(
        bundle
            .criteria
            .iter()
            .find(|result| result.id() == CriterionId::from_parts(1, 6))
            .unwrap()
            .status(),
        CriterionStatus::Unsatisfied
    );

    let (repository, runtime) = artifact_fixture("artifact-arm64-absent");
    let runner = FixtureCommandRunner::successful();
    runner.captures.borrow_mut()[1].stdout = b"x86_64\n".to_vec();

    let bundle = probe_artifact_with_runner(&fixed_manifest(), &repository.path, &runtime, &runner);

    assert_eq!(bundle.gates[0].status(), GateStatus::Pass);
    assert_eq!(bundle.gates[1].status(), GateStatus::Fail);
    assert_eq!(
        bundle
            .criteria
            .iter()
            .find(|result| result.id() == CriterionId::from_parts(2, 2))
            .unwrap()
            .status(),
        CriterionStatus::Unsatisfied
    );
}

#[test]
fn artifact_probe_path_or_provenance_mismatch_runs_no_commands() {
    let (repository, runtime) = artifact_fixture("artifact-path");
    let alternate = repository.path.join("alternate/libsciter.dylib");
    fs::create_dir_all(alternate.parent().unwrap()).unwrap();
    fs::write(&alternate, b"fixture runtime").unwrap();

    let runner = FixtureCommandRunner::successful();
    let path_bundle =
        probe_artifact_with_runner(&fixed_manifest(), &repository.path, &alternate, &runner);
    assert!(runner.calls().is_empty());
    assert_eq!(path_bundle.gates[0].status(), GateStatus::Fail);
    assert_eq!(path_bundle.gates[1].status(), GateStatus::NotRun);

    let runner = FixtureCommandRunner::successful();
    let mut manifest = fixed_manifest();
    manifest.commit = "031ec0f726bdbe5d0402ad647f3b34feef84654e".to_owned();
    let provenance_bundle =
        probe_artifact_with_runner(&manifest, &repository.path, &runtime, &runner);
    assert!(runner.calls().is_empty());
    assert_eq!(provenance_bundle.gates[0].status(), GateStatus::Fail);
}

#[test]
fn artifact_probe_missing_or_nonzero_command_is_fail_closed_and_keeps_raw_status() {
    for status in [
        CommandStatus::NotStarted("command not found".to_owned()),
        CommandStatus::Exited(2),
    ] {
        let (repository, runtime) = artifact_fixture("artifact-command-failure");
        let runner = FixtureCommandRunner::successful();
        runner.captures.borrow_mut()[1].status = status.clone();

        let bundle =
            probe_artifact_with_runner(&fixed_manifest(), &repository.path, &runtime, &runner);

        assert_ne!(bundle.gates[1].status(), GateStatus::Pass);
        assert_eq!(bundle.command_captures[1].status, status);
        assert!(bundle.raw_artifacts.iter().any(|artifact| {
            artifact.relative_path == Path::new("metadata/lipo.status")
                && !artifact.bytes.is_empty()
        }));
    }
}

#[test]
fn every_artifact_command_failure_is_typed_and_cannot_pass_an_owned_gate() {
    let affected_criteria = [(1, 5), (2, 1), (2, 5), (2, 6), (2, 7), (2, 8)];
    let artifact_names = [
        "shasum",
        "lipo",
        "otool-load",
        "otool-dependencies",
        "otool-install-name",
        "codesign",
    ];
    for failure_status in [
        CommandStatus::NotStarted("command not found".to_owned()),
        CommandStatus::Exited(23),
    ] {
        for (command_index, (requirement, criterion)) in affected_criteria.into_iter().enumerate() {
            let (repository, runtime) = artifact_fixture("artifact-each-command-failure");
            let runner = FixtureCommandRunner::successful();
            let stdout = format!("failure stdout {command_index}\n");
            let stderr = format!("failure stderr {command_index}\n");
            let mut captures = runner.captures.borrow_mut();
            captures[command_index].stdout = stdout.as_bytes().to_vec();
            captures[command_index].stderr = stderr.as_bytes().to_vec();
            captures[command_index].status = failure_status.clone();
            drop(captures);

            let bundle =
                probe_artifact_with_runner(&fixed_manifest(), &repository.path, &runtime, &runner);

            assert_eq!(
                bundle
                    .criteria
                    .iter()
                    .find(|result| {
                        result.id() == CriterionId::from_parts(requirement, criterion)
                    })
                    .unwrap()
                    .status(),
                CriterionStatus::NotRun,
                "command index {command_index}"
            );
            let owned_gate = if requirement == 1 {
                GateId::Artifact
            } else {
                GateId::Platform
            };
            assert_ne!(
                bundle
                    .gates
                    .iter()
                    .find(|gate| gate.id() == owned_gate)
                    .unwrap()
                    .status(),
                GateStatus::Pass
            );
            assert_eq!(
                bundle.command_captures[command_index].status,
                failure_status
            );
            for (suffix, expected) in [
                ("stdout", stdout.as_bytes()),
                ("stderr", stderr.as_bytes()),
                ("status", format!("{failure_status:?}\n").as_bytes()),
            ] {
                let path = PathBuf::from(format!(
                    "metadata/{}.{suffix}",
                    artifact_names[command_index]
                ));
                assert_eq!(
                    bundle
                        .raw_artifacts
                        .iter()
                        .find(|artifact| artifact.relative_path == path)
                        .unwrap()
                        .bytes,
                    expected,
                    "{}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn system_runner_maps_only_allowed_identifiers_to_fixed_absolute_paths() {
    assert_eq!(
        SystemCommandRunner::program_path("shasum"),
        Some("/usr/bin/shasum")
    );
    assert_eq!(
        SystemCommandRunner::program_path("lipo"),
        Some("/usr/bin/lipo")
    );
    assert_eq!(
        SystemCommandRunner::program_path("otool"),
        Some("/usr/bin/otool")
    );
    assert_eq!(
        SystemCommandRunner::program_path("codesign"),
        Some("/usr/bin/codesign")
    );
    assert_eq!(SystemCommandRunner::program_path("sh"), None);
    assert_eq!(SystemCommandRunner::program_path("/tmp/shasum"), None);

    let rejected = SystemCommandRunner.run("sh", &["-c".into(), "exit 0".into()]);
    assert!(matches!(rejected.status, CommandStatus::NotStarted(_)));
    assert_eq!(rejected.program, "sh");
}

#[test]
fn minimum_macos_parser_supports_legacy_commands_and_ignores_unrelated_versions() {
    let output = "runtime (architecture x86_64):\n      cmd LC_VERSION_MIN_MACOSX\n  cmdsize 16\n  version 10.13\n      sdk 10.15\nLoad command 10\n      cmd LC_SOURCE_VERSION\n  version 99.88\nruntime (architecture arm64):\n      cmd LC_BUILD_VERSION\n platform 1\n    minos 11.5\n      sdk 26.4\n     tool 3\n  version 1266.8\n";

    assert_eq!(
        parse_minimum_versions(output),
        [
            ("x86_64".to_owned(), "10.13".to_owned()),
            ("arm64".to_owned(), "11.5".to_owned()),
        ]
    );
}

#[test]
fn host_snapshot_and_same_revision_headers_are_returned_in_the_probe_bundle() {
    let (repository, runtime) = artifact_fixture("host-headers");
    write_header_fixture(&repository, [6, 0, 3, 18], 10);
    let runner = FixtureCommandRunner::successful();
    runner.captures.borrow_mut().extend([
        capture("sysctl", "Mac15,7\n", "sysctl diagnostic\n", 0),
        capture("sw_vers", "15.6\n", "sw_vers diagnostic\n", 0),
        capture("uname", "arm64\n", "uname diagnostic\n", 0),
    ]);

    let bundle = probe_artifact_with_runner_at(
        &fixed_manifest(),
        &repository.path,
        &runtime,
        &runner,
        UNIX_EPOCH,
    );

    assert_eq!(bundle.started_at_utc, "1970-01-01T00:00:00Z");
    assert_eq!(bundle.host.hardware, "Mac15,7");
    assert_eq!(bundle.host.macos_version, "15.6");
    assert_eq!(bundle.host.process_architecture, "arm64");
    let HeaderProbeOutcome::Verified(headers) = bundle.header_evidence else {
        panic!("same-revision headers should be verified");
    };
    assert_eq!(headers.commit, fixed_manifest().commit);
    assert_eq!(headers.engine_version, [6, 0, 3, 18]);
    assert_eq!(headers.api_version, 10);
    for id in [(2, 3), (2, 4), (3, 1), (3, 2), (4, 1), (4, 2)] {
        assert_eq!(
            bundle
                .criteria
                .iter()
                .find(|result| result.id() == CriterionId::from_parts(id.0, id.1))
                .unwrap()
                .status(),
            CriterionStatus::Satisfied,
            "criterion {}.{}",
            id.0,
            id.1
        );
    }
    assert_eq!(
        bundle
            .criteria
            .iter()
            .find(|result| result.id() == CriterionId::from_parts(4, 7))
            .unwrap()
            .status(),
        CriterionStatus::NotApplicable
    );
    assert!(bundle.raw_artifacts.iter().any(|artifact| {
        artifact.relative_path == Path::new("metadata/host/sysctl-hardware.stderr")
            && artifact.bytes == b"sysctl diagnostic\n"
    }));
    assert!(bundle.raw_artifacts.iter().any(|artifact| {
        artifact.relative_path == Path::new("metadata/headers/sciter-version.h")
            && artifact.bytes.starts_with(b"#define SCITER_VERSION_0")
    }));
}

#[test]
fn missing_headers_are_observable_not_run_results_without_network_fallback() {
    let (repository, runtime) = artifact_fixture("missing-headers");
    let runner = FixtureCommandRunner::successful();
    runner.captures.borrow_mut().extend([
        capture("sysctl", "Mac15,7\n", "", 0),
        capture("sw_vers", "15.6\n", "", 0),
        capture("uname", "arm64\n", "", 0),
    ]);

    let bundle = probe_artifact_with_runner_at(
        &fixed_manifest(),
        &repository.path,
        &runtime,
        &runner,
        UNIX_EPOCH,
    );

    assert!(matches!(
        bundle.header_evidence,
        HeaderProbeOutcome::NotRun { .. }
    ));
    for id in [(4, 1), (4, 2), (4, 7)] {
        assert_eq!(
            bundle
                .criteria
                .iter()
                .find(|result| result.id() == CriterionId::from_parts(id.0, id.1))
                .unwrap()
                .status(),
            CriterionStatus::NotRun
        );
    }
    assert_eq!(
        bundle
            .gates
            .iter()
            .find(|gate| gate.id() == GateId::Abi)
            .unwrap()
            .status(),
        GateStatus::NotRun
    );
    assert_eq!(
        runner.calls().len(),
        9,
        "header absence must not trigger commands"
    );
}

#[test]
fn header_binding_or_manifest_disagreement_fails_closed() {
    let (repository, runtime) = artifact_fixture("header-mismatch");
    write_header_fixture(&repository, [6, 0, 3, 19], 10);
    let runner = FixtureCommandRunner::successful();
    runner.captures.borrow_mut().extend([
        capture("sysctl", "Mac15,7\n", "", 0),
        capture("sw_vers", "15.6\n", "", 0),
        capture("uname", "arm64\n", "", 0),
    ]);

    let bundle = probe_artifact_with_runner_at(
        &fixed_manifest(),
        &repository.path,
        &runtime,
        &runner,
        UNIX_EPOCH,
    );

    assert!(matches!(
        bundle.header_evidence,
        HeaderProbeOutcome::Disagreed { .. }
    ));
    assert_eq!(
        bundle
            .criteria
            .iter()
            .find(|result| result.id() == CriterionId::from_parts(4, 1))
            .unwrap()
            .status(),
        CriterionStatus::Unsatisfied
    );
    assert_eq!(
        bundle
            .criteria
            .iter()
            .find(|result| result.id() == CriterionId::from_parts(4, 7))
            .unwrap()
            .status(),
        CriterionStatus::Unsatisfied
    );
}

#[test]
fn ambiguous_authoritative_header_define_is_not_accepted() {
    let (repository, runtime) = artifact_fixture("ambiguous-header");
    write_header_fixture(&repository, [6, 0, 3, 18], 10);
    let version_header = repository
        .path
        .join("vendor/sciter-js-sdk-main/include/sciter-version.h");
    let mut contents = fs::read_to_string(&version_header).unwrap();
    contents.push_str("#define SCITER_VERSION_0 6\n");
    fs::write(version_header, contents).unwrap();
    let runner = FixtureCommandRunner::successful();
    runner.captures.borrow_mut().extend([
        capture("sysctl", "Mac15,7\n", "", 0),
        capture("sw_vers", "15.6\n", "", 0),
        capture("uname", "arm64\n", "", 0),
    ]);

    let bundle = probe_artifact_with_runner_at(
        &fixed_manifest(),
        &repository.path,
        &runtime,
        &runner,
        UNIX_EPOCH,
    );

    assert!(matches!(
        bundle.header_evidence,
        HeaderProbeOutcome::NotRun { .. }
    ));
    assert_eq!(
        bundle
            .criteria
            .iter()
            .find(|result| result.id() == CriterionId::from_parts(4, 1))
            .unwrap()
            .status(),
        CriterionStatus::NotRun
    );
}

#[test]
fn host_commands_use_only_fixed_absolute_production_paths() {
    assert_eq!(
        SystemCommandRunner::program_path("sysctl"),
        Some("/usr/sbin/sysctl")
    );
    assert_eq!(
        SystemCommandRunner::program_path("sw_vers"),
        Some("/usr/bin/sw_vers")
    );
    assert_eq!(
        SystemCommandRunner::program_path("uname"),
        Some("/usr/bin/uname")
    );
}

#[test]
fn unestablished_header_revision_is_not_run_even_when_files_exist() {
    let (repository, runtime) = artifact_fixture("header-identity");
    write_header_fixture(&repository, [6, 0, 3, 18], 10);
    let mut manifest = fixed_manifest();
    manifest.version_header_source = manifest.version_header_source.replace(
        "e31ec0f726bdbe5d0402ad647f3b34feef84654e",
        "031ec0f726bdbe5d0402ad647f3b34feef84654e",
    );
    let runner = FixtureCommandRunner::successful();
    runner.captures.borrow_mut().extend([
        capture("sysctl", "Mac15,7\n", "", 0),
        capture("sw_vers", "15.6\n", "", 0),
        capture("uname", "arm64\n", "", 0),
    ]);

    let bundle =
        probe_artifact_with_runner_at(&manifest, &repository.path, &runtime, &runner, UNIX_EPOCH);

    assert!(matches!(
        bundle.header_evidence,
        HeaderProbeOutcome::NotRun { .. }
    ));
    assert_eq!(
        bundle
            .criteria
            .iter()
            .find(|result| result.id() == CriterionId::from_parts(4, 1))
            .unwrap()
            .status(),
        CriterionStatus::NotRun
    );
}
