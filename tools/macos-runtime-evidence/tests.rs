use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
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
use crate::sciter::{
    host_context_drop_count_for_tests, reset_context_drop_counts_for_tests, AbiField, AppCommand,
    DebugCallbackContext, DynamicLoader, HostCallbackContext, HostCallbackFailure, LifecycleEntry,
    LifecycleError, RuntimeAbiError, RuntimeLoadError, SciterRuntime, ThreadContext, WindowFlags,
    WindowState, RTLD_LOCAL, RTLD_NOW,
};

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

#[derive(Debug, Eq, PartialEq)]
struct ParsedChildLine {
    record: String,
    fields: BTreeMap<String, String>,
}

fn parse_child_protocol(stdout: &[u8]) -> Result<Vec<ParsedChildLine>, String> {
    std::str::from_utf8(stdout)
        .map_err(|error| error.to_string())?
        .lines()
        .filter(|line| line.starts_with("MDLUMA_EVIDENCE\t"))
        .map(parse_child_protocol_line)
        .collect()
}

fn parse_child_protocol_line(line: &str) -> Result<ParsedChildLine, String> {
    let columns: Vec<_> = line.split('\t').collect();
    if columns.len() < 4
        || columns[0] != "MDLUMA_EVIDENCE"
        || columns[1] != "1"
        || columns[2] != "api_abi"
    {
        return Err("invalid protocol envelope".to_owned());
    }
    let expected_keys: &[&str] = match columns[3] {
        "progress" => &["stage", "state", "actual", "expected"],
        "result" => &[
            "status",
            "actual_api",
            "expected_api",
            "actual_engine",
            "expected_engine",
            "scope",
            "lifecycle_abi_validated",
            "process_architecture",
            "thread_context",
            "failure_stage",
        ],
        _ => return Err("invalid protocol record type".to_owned()),
    };
    if columns.len() != expected_keys.len() + 4 {
        return Err("invalid protocol field count".to_owned());
    }
    let mut fields = BTreeMap::new();
    for (column, expected_key) in columns[4..].iter().zip(expected_keys) {
        let (key, value) = column
            .split_once('=')
            .ok_or_else(|| "protocol field is not key=value".to_owned())?;
        if key != *expected_key || value.is_empty() || value.contains('=') {
            return Err("invalid protocol key or value".to_owned());
        }
        fields.insert(key.to_owned(), decode_protocol_value(value)?);
    }
    Ok(ParsedChildLine {
        record: columns[3].to_owned(),
        fields,
    })
}

fn decode_protocol_value(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            if bytes[index].is_ascii_control() {
                return Err("unescaped protocol control byte".to_owned());
            }
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return Err("truncated protocol escape".to_owned());
        }
        let escaped = match &bytes[index + 1..=index + 2] {
            b"09" => b'\t',
            b"0A" => b'\n',
            b"0D" => b'\r',
            b"25" => b'%',
            b"3D" => b'=',
            _ => return Err("unsupported protocol escape".to_owned()),
        };
        decoded.push(escaped);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|error| error.to_string())
}

fn compile_api_abi_fixture(label: &str) -> (TestDirectory, PathBuf) {
    let directory = TestDirectory::new(label);
    let source_path = directory.path.join("api-abi-fixture.rs");
    let binary_path = directory.path.join("api-abi-fixture");
    let source = format!(
        r#"
#[path = {model_path:?}]
mod model;
#[path = {manifest_path:?}]
mod manifest;
#[path = {sciter_path:?}]
mod sciter;
#[path = {harness_path:?}]
mod harness;

fn main() {{
    let manifest = manifest::parse_artifact_manifest({manifest:?}).unwrap();
    let mode = std::env::args().nth(1).unwrap();
    let successful_probe = |emit: &mut dyn FnMut(harness::ProbeEvent)| {{
        for (stage, actual, expected) in [
            (harness::HarnessStage::RuntimeLoad, "loaded", "loaded"),
            (harness::HarnessStage::SciterApiExport, "resolved", "resolved"),
            (harness::HarnessStage::ApiTable, "non_null", "non_null"),
            (harness::HarnessStage::ApiVersion, "10", "10"),
            (harness::HarnessStage::SciterVersionCall, "6.0.3.18", "6.0.3.18"),
        ] {{
            emit(harness::ProbeEvent::entered(stage));
            emit(harness::ProbeEvent::completed(stage, actual, expected));
        }}
        Ok(harness::ApiAbiValues::new(10, [6, 0, 3, 18]))
    }};
    let result = match mode.as_str() {{
        "success" | "success-nonzero" | "success-abort" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "arm64",
            true,
            successful_probe,
        ),
        "api-mismatch" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "arm64",
            true,
            |emit| {{
                successful_probe(emit)?;
                Ok(harness::ApiAbiValues::new(11, [6, 0, 3, 18]))
            }},
        ),
        "engine-mismatch" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "arm64",
            true,
            |emit| {{
                successful_probe(emit)?;
                Ok(harness::ApiAbiValues::new(10, [6, 0, 3, 19]))
            }},
        ),
        "load-failure" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "arm64",
            true,
            |emit| {{
                emit(harness::ProbeEvent::entered(harness::HarnessStage::RuntimeLoad));
                Err(harness::ApiAbiFailure::new(harness::HarnessStage::RuntimeLoad, "fixture load failure"))
            }},
        ),
        "export-failure" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "arm64",
            true,
            |emit| {{
                emit(harness::ProbeEvent::entered(harness::HarnessStage::SciterApiExport));
                Err(harness::ApiAbiFailure::new(harness::HarnessStage::SciterApiExport, "fixture export failure"))
            }},
        ),
        "table-failure" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "arm64",
            true,
            |emit| {{
                emit(harness::ProbeEvent::entered(harness::HarnessStage::ApiTable));
                Err(harness::ApiAbiFailure::new(harness::HarnessStage::ApiTable, "fixture null table"))
            }},
        ),
        "architecture-failure" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "x86_64",
            true,
            |_| panic!("runtime probe must not run for a non-arm64 process"),
        ),
        "thread-failure" => harness::run_api_abi_child_with(
            std::path::Path::new("/fixed/libsciter.dylib"),
            &manifest,
            "arm64",
            false,
            |_| panic!("runtime probe must not run off the main thread"),
        ),
        _ => panic!("unknown fixture mode"),
    }};
    if result.is_err() {{
        std::process::exit(1);
    }}
    match mode.as_str() {{
        "success-nonzero" => std::process::exit(23),
        "success-abort" => std::process::abort(),
        _ => {{}},
    }}
}}
"#,
        model_path = toolkit_dir().join("model.rs"),
        manifest_path = toolkit_dir().join("manifest.rs"),
        sciter_path = toolkit_dir().join("sciter.rs"),
        harness_path = toolkit_dir().join("harness.rs"),
        manifest = ARTIFACT_MANIFEST,
    );
    fs::write(&source_path, source).unwrap();
    compile_rust_source(&source_path, &binary_path);
    (directory, binary_path)
}

fn compile_rust_source(source: &Path, binary: &Path) {
    let mut compiler = if let Some(rustc) = std::env::var_os("RUSTC") {
        Command::new(rustc)
    } else {
        let mut command = Command::new("mise");
        command.args(["exec", "rust@stable", "--", "rustc"]);
        command
    };
    let compilation = compiler
        .args(["--edition=2021"])
        .arg(source)
        .arg("-o")
        .arg(binary)
        .output()
        .unwrap();
    assert!(
        compilation.status.success(),
        "fixture compilation failed: {}",
        String::from_utf8_lossy(&compilation.stderr)
    );
}

#[test]
fn api_abi_child_success_emits_fixed_order_versioned_protocol_and_exits_zero() {
    let (_directory, binary) = compile_api_abi_fixture("api-abi-success");
    let output = Command::new(binary).arg("success").output().unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let protocol = parse_child_protocol(&output.stdout).unwrap();
    assert_eq!(
        protocol
            .iter()
            .filter(|line| line.record == "progress")
            .map(|line| (line.fields["stage"].as_str(), line.fields["state"].as_str()))
            .collect::<Vec<_>>(),
        [
            ("runtime_load", "entered"),
            ("runtime_load", "completed"),
            ("sciter_api_export", "entered"),
            ("sciter_api_export", "completed"),
            ("api_table", "entered"),
            ("api_table", "completed"),
            ("api_version", "entered"),
            ("api_version", "completed"),
            ("sciter_version_call", "entered"),
            ("sciter_version_call", "completed"),
        ]
    );
    let result = protocol.last().unwrap();
    assert_eq!(result.record, "result");
    assert_eq!(result.fields["status"], "success_candidate");
    assert_eq!(result.fields["actual_api"], "10");
    assert_eq!(result.fields["actual_engine"], "6.0.3.18");
    assert_eq!(result.fields["scope"], "api_version+SciterVersion");
    assert!(!String::from_utf8(output.stdout)
        .unwrap()
        .contains("gate=pass"));
}

#[test]
fn child_protocol_parser_rejects_bad_envelopes_fields_and_escaping() {
    for invalid in [
        "OTHER\t1\tapi_abi\tprogress\tstage=x\tstate=entered\tactual=x\texpected=x",
        "MDLUMA_EVIDENCE\t2\tapi_abi\tprogress\tstage=x\tstate=entered\tactual=x\texpected=x",
        "MDLUMA_EVIDENCE\t1\tapi_abi\tprogress\tstage=x\tstate=entered\tactual=x",
        "MDLUMA_EVIDENCE\t1\tapi_abi\tprogress\tstate=entered\tstage=x\tactual=x\texpected=x",
        "MDLUMA_EVIDENCE\t1\tapi_abi\tprogress\tstage=x\tstate=entered\tactual=bad%0x\texpected=x",
        "MDLUMA_EVIDENCE\t1\tapi_abi\tprogress\tstage=x\tstate=entered\tactual=a=b\texpected=x",
    ] {
        assert!(parse_child_protocol_line(invalid).is_err(), "{invalid}");
    }
    let parsed = parse_child_protocol_line(
        "MDLUMA_EVIDENCE\t1\tapi_abi\tprogress\tstage=x\tstate=entered\tactual=a%25b%3Dc\texpected=x",
    )
    .unwrap();
    assert_eq!(parsed.fields["actual"], "a%b=c");
}

#[test]
fn api_abi_child_mismatch_is_nonzero_and_never_reports_pass_gates() {
    let (_directory, binary) = compile_api_abi_fixture("api-abi-mismatch");
    let output = Command::new(binary).arg("api-mismatch").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(output.status.code(), Some(1));
    parse_child_protocol(stdout.as_bytes()).unwrap();
    assert!(stdout.contains("stage=api_version\tstate=failed\tactual=11\texpected=10"));
    assert!(stdout.contains("failure_stage=api_version"));
    assert!(!stdout.contains("api_gate="));
    assert!(!stdout.contains("abi_gate="));
    assert_eq!(
        stderr,
        "api-abi child failed: stage=api_version diagnostic=actual API version 11 did not match expected 10\n"
    );
}

#[test]
fn api_abi_child_load_and_thread_failures_are_nonzero_with_separate_diagnostics() {
    let (_directory, binary) = compile_api_abi_fixture("api-abi-failures");

    for (mode, stage, diagnostic) in [
        ("load-failure", "runtime_load", "fixture load failure"),
        (
            "export-failure",
            "sciter_api_export",
            "fixture export failure",
        ),
        ("table-failure", "api_table", "fixture null table"),
        (
            "architecture-failure",
            "process_architecture",
            "child process architecture x86_64 did not match expected arm64",
        ),
        (
            "thread-failure",
            "thread_context",
            "child must run on the process main thread",
        ),
    ] {
        let output = Command::new(&binary).arg(mode).output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(1));
        parse_child_protocol(stdout.as_bytes()).unwrap();
        assert!(stdout.contains(&format!("failure_stage={stage}")));
        assert!(!stdout.contains("api_gate="));
        assert!(!stdout.contains("abi_gate="));
        assert_eq!(
            stderr,
            format!("api-abi child failed: stage={stage} diagnostic={diagnostic}\n")
        );
    }
}

#[test]
fn api_abi_child_engine_mismatch_is_nonzero_and_reports_both_versions() {
    let (_directory, binary) = compile_api_abi_fixture("api-abi-engine-mismatch");
    let output = Command::new(binary)
        .arg("engine-mismatch")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout
        .contains("stage=sciter_version_call\tstate=failed\tactual=6.0.3.19\texpected=6.0.3.18"));
    assert!(stdout.contains("failure_stage=sciter_version_call"));
    assert!(!stdout.contains("api_gate="));
    assert!(!stdout.contains("abi_gate="));
    assert_eq!(
        stderr,
        "api-abi child failed: stage=sciter_version_call diagnostic=actual engine version 6.0.3.19 did not match expected 6.0.3.18\n"
    );
}

#[test]
fn successful_probe_then_abnormal_exit_keeps_last_stage_without_gate_pass() {
    let (_directory, binary) = compile_api_abi_fixture("api-abi-abnormal-after-probe");

    for mode in ["success-nonzero", "success-abort"] {
        let output = Command::new(&binary).arg(mode).output().unwrap();
        if mode == "success-nonzero" {
            assert_eq!(output.status.code(), Some(23));
        } else {
            assert_eq!(output.status.signal(), Some(6));
        }
        let stdout = String::from_utf8(output.stdout).unwrap();
        let protocol = parse_child_protocol(stdout.as_bytes()).unwrap();
        let last_progress = protocol
            .iter()
            .rev()
            .find(|line| line.record == "progress")
            .unwrap();
        assert_eq!(last_progress.fields["stage"], "sciter_version_call");
        assert_eq!(last_progress.fields["state"], "completed");
        assert!(protocol.iter().any(|line| line
            .fields
            .get("status")
            .is_some_and(|value| value == "success_candidate")));
        assert!(!stdout.contains("api_gate="));
        assert!(!stdout.contains("abi_gate="));
        assert!(!stdout.contains("gate=pass"));
    }
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

thread_local! {
    static VERSION_SELECTORS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "C" fn synthetic_sciter_version(selector: u32) -> u32 {
    VERSION_SELECTORS.with(|selectors| selectors.borrow_mut().push(selector));
    [6, 0, 3, 18]
        .get(selector as usize)
        .copied()
        .unwrap_or(u32::MAX)
}

fn synthetic_api_table(
    api_version: u32,
    version: Option<unsafe extern "C" fn(u32) -> u32>,
) -> Box<crate::sciter::bindings::ISciterAPI> {
    let mut table: Box<crate::sciter::bindings::ISciterAPI> =
        Box::new(unsafe { std::mem::zeroed() });
    table.version = api_version;
    table.SciterVersion = version;
    table
}

thread_local! {
    static LIFECYCLE_CALLS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static LIFECYCLE_COMMANDS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static LIFECYCLE_EXEC_RESULT: RefCell<Option<(u32, crate::sciter::bindings::INT_PTR)>> = const { RefCell::new(None) };
    static SYNTHETIC_MAIN_THREAD: RefCell<bool> = const { RefCell::new(true) };
}

fn synthetic_main_thread_check() -> bool {
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow())
}

unsafe extern "C" fn synthetic_sciter_exec(
    command: u32,
    _p1: crate::sciter::bindings::UINT_PTR,
    _p2: crate::sciter::bindings::UINT_PTR,
) -> crate::sciter::bindings::INT_PTR {
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().push("exec"));
    LIFECYCLE_COMMANDS.with(|commands| commands.borrow_mut().push(command));
    LIFECYCLE_EXEC_RESULT.with(|result| {
        result
            .borrow()
            .filter(|(expected, _)| *expected == command)
            .map_or_else(|| if command == 6 { 1 } else { 0 }, |(_, raw)| raw)
    })
}

unsafe extern "C" fn synthetic_create_window(
    _flags: u32,
    _frame: crate::sciter::bindings::LPRECT,
    _reserved1: *mut std::ffi::c_void,
    _reserved2: *mut std::ffi::c_void,
    _parent: crate::sciter::bindings::HWND,
) -> crate::sciter::bindings::HWND {
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().push("create"));
    std::ptr::NonNull::<crate::sciter::bindings::HWND__>::dangling().as_ptr()
}

unsafe extern "C" fn synthetic_set_callback(
    _window: crate::sciter::bindings::HWND,
    callback: crate::sciter::bindings::LPSciterHostCallback,
    context: *mut std::ffi::c_void,
) {
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().push("callback"));
    let mut notification = crate::sciter::bindings::SCITER_CALLBACK_NOTIFICATION {
        code: 5,
        hwnd: std::ptr::null_mut(),
    };
    unsafe { callback.unwrap()(&mut notification, context) };
}

unsafe extern "C" fn synthetic_set_callback_without_notification(
    _window: crate::sciter::bindings::HWND,
    _callback: crate::sciter::bindings::LPSciterHostCallback,
    _context: *mut std::ffi::c_void,
) {
}

unsafe extern "C" fn synthetic_load_html(
    _window: crate::sciter::bindings::HWND,
    _html: *const u8,
    _length: u32,
    _base_url: *const u16,
) -> i32 {
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().push("html"));
    1
}

unsafe extern "C" fn synthetic_window_exec(
    _window: crate::sciter::bindings::HWND,
    command: u32,
    _p1: crate::sciter::bindings::UINT_PTR,
    _p2: crate::sciter::bindings::UINT_PTR,
) -> crate::sciter::bindings::INT_PTR {
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().push("state"));
    if command == 2 {
        2
    } else {
        1
    }
}

unsafe extern "C" fn synthetic_setup_debug_output(
    _window: crate::sciter::bindings::HWND,
    callback_context: *mut std::ffi::c_void,
    callback: crate::sciter::bindings::DEBUG_OUTPUT_PROC,
) {
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().push("debug"));
    let text = [b'o' as u16, b'k' as u16];
    unsafe { callback.unwrap()(callback_context, 0, 0, text.as_ptr(), text.len() as u32) };
}

fn synthetic_lifecycle_api_table() -> Box<crate::sciter::bindings::ISciterAPI> {
    let mut table = synthetic_api_table(10, Some(synthetic_sciter_version));
    table.SciterExec = Some(synthetic_sciter_exec);
    table.SciterCreateWindow = Some(synthetic_create_window);
    table.SciterSetCallback = Some(synthetic_set_callback);
    table.SciterLoadHtml = Some(synthetic_load_html);
    table.SciterWindowExec = Some(synthetic_window_exec);
    table.SciterSetupDebugOutput = Some(synthetic_setup_debug_output);
    table
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
fn committed_bindings_abi_smoke_matches_manifest_and_limits_its_claim() {
    VERSION_SELECTORS.with(|selectors| selectors.borrow_mut().clear());
    let table = synthetic_api_table(10, Some(synthetic_sciter_version));
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&*table) };

    let result = runtime
        .abi_smoke_with_main_thread_check(&fixed_manifest(), || true)
        .unwrap();

    assert_eq!(result.actual_api_version(), 10);
    assert_eq!(result.expected_api_version(), 10);
    assert!(result.api_matches());
    assert_eq!(result.actual_engine_version(), [6, 0, 3, 18]);
    assert_eq!(result.expected_engine_version(), [6, 0, 3, 18]);
    assert!(result.engine_matches());
    assert!(result.version_call_returned());
    assert_eq!(result.process_architecture(), "arm64");
    assert_eq!(result.thread_context(), ThreadContext::Main);
    assert_eq!(
        result.validated_fields(),
        &[AbiField::ApiVersion, AbiField::SciterVersion]
    );
    assert!(!result.validates_lifecycle_api());
    VERSION_SELECTORS.with(|selectors| assert_eq!(*selectors.borrow(), [0, 1, 2, 3]));
}

#[test]
fn committed_bindings_abi_smoke_reports_api_and_engine_mismatches() {
    let table = synthetic_api_table(11, Some(synthetic_sciter_version));
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&*table) };
    let mut manifest = fixed_manifest();
    manifest.engine_version = [6, 0, 3, 19];

    let result = runtime
        .abi_smoke_with_main_thread_check(&manifest, || true)
        .unwrap();

    assert!(!result.api_matches());
    assert!(!result.engine_matches());
    assert_eq!(result.actual_api_version(), 11);
    assert_eq!(result.actual_engine_version(), [6, 0, 3, 18]);
}

#[test]
fn committed_bindings_abi_smoke_rejects_null_version_entry_before_call() {
    let table = synthetic_api_table(10, None);
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&*table) };

    assert_eq!(
        runtime.abi_smoke_with_main_thread_check(&fixed_manifest(), || true),
        Err(RuntimeAbiError::NullSciterVersion)
    );
}

#[test]
fn committed_bindings_abi_smoke_enforces_main_thread_without_calling_sciter() {
    VERSION_SELECTORS.with(|selectors| selectors.borrow_mut().clear());
    let table = synthetic_api_table(10, Some(synthetic_sciter_version));
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&*table) };

    assert_eq!(
        runtime.abi_smoke_with_main_thread_check(&fixed_manifest(), || false),
        Err(RuntimeAbiError::NotMainThread)
    );
    VERSION_SELECTORS.with(|selectors| assert!(selectors.borrow().is_empty()));
}

#[test]
fn lifecycle_api_readiness_identifies_each_required_missing_entry() {
    let cases = [
        LifecycleEntry::SciterExec,
        LifecycleEntry::SciterCreateWindow,
        LifecycleEntry::SciterSetCallback,
        LifecycleEntry::SciterLoadHtml,
        LifecycleEntry::SciterWindowExec,
        LifecycleEntry::SciterSetupDebugOutput,
    ];

    for missing in cases {
        let mut table = synthetic_lifecycle_api_table();
        match missing {
            LifecycleEntry::SciterExec => table.SciterExec = None,
            LifecycleEntry::SciterCreateWindow => table.SciterCreateWindow = None,
            LifecycleEntry::SciterSetCallback => table.SciterSetCallback = None,
            LifecycleEntry::SciterLoadHtml => table.SciterLoadHtml = None,
            LifecycleEntry::SciterWindowExec => table.SciterWindowExec = None,
            LifecycleEntry::SciterSetupDebugOutput => table.SciterSetupDebugOutput = None,
        }
        let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
        assert!(matches!(
            runtime.lifecycle_api_with_main_thread_check(synthetic_main_thread_check),
            Err(LifecycleError::MissingEntry(entry)) if entry == missing
        ));
    }
}

#[test]
fn lifecycle_calls_reject_non_main_thread_before_entering_sciter() {
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().clear());
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
    let table = synthetic_lifecycle_api_table();
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
    let api = runtime
        .lifecycle_api_with_main_thread_check(synthetic_main_thread_check)
        .unwrap();
    let window = api.create_window(WindowFlags::MAIN, None, None).unwrap();
    LIFECYCLE_CALLS.with(|calls| calls.borrow_mut().clear());
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = false);

    assert_eq!(
        api.exec(AppCommand::Init, 0, 0),
        Err(LifecycleError::NotMainThread)
    );
    assert_eq!(
        api.create_window(WindowFlags::MAIN, None, None),
        Err(LifecycleError::NotMainThread)
    );
    assert_eq!(
        api.load_html(window, b"<html></html>", None),
        Err(LifecycleError::NotMainThread)
    );
    assert_eq!(
        api.set_window_state(window, WindowState::Shown, false),
        Err(LifecycleError::NotMainThread)
    );
    assert_eq!(api.window_state(window), Err(LifecycleError::NotMainThread));
    assert!(matches!(
        api.register_host_callback(window, HostCallbackContext::new()),
        Err(LifecycleError::NotMainThread)
    ));
    assert!(matches!(
        api.register_debug_output(None, DebugCallbackContext::new("MDLUMA_EVIDENCE\t")),
        Err(LifecycleError::NotMainThread)
    ));
    LIFECYCLE_CALLS.with(|calls| assert!(calls.borrow().is_empty()));
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
}

#[test]
fn host_context_address_is_stable_and_destroyed_context_drops_only_after_callback_returns() {
    reset_context_drop_counts_for_tests();
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
    let table = synthetic_lifecycle_api_table();
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
    let api = runtime
        .lifecycle_api_with_main_thread_check(synthetic_main_thread_check)
        .unwrap();
    let window = api.create_window(WindowFlags::MAIN, None, None).unwrap();
    let context = HostCallbackContext::new();
    let address = context.stable_address();

    let registered = api.register_host_callback(window, context).unwrap();

    assert_eq!(registered.stable_address(), address);
    assert!(registered.destroyed().unwrap());
    assert_eq!(host_context_drop_count_for_tests(), 0);
    drop(registered);
    assert_eq!(host_context_drop_count_for_tests(), 1);
}

#[test]
fn registered_host_context_is_not_freed_before_destroy_notification() {
    reset_context_drop_counts_for_tests();
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
    let mut table = synthetic_lifecycle_api_table();
    table.SciterSetCallback = Some(synthetic_set_callback_without_notification);
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
    let api = runtime
        .lifecycle_api_with_main_thread_check(synthetic_main_thread_check)
        .unwrap();
    let window = api.create_window(WindowFlags::MAIN, None, None).unwrap();

    drop(
        api.register_host_callback(window, HostCallbackContext::new())
            .unwrap(),
    );

    assert_eq!(host_context_drop_count_for_tests(), 0);
}

#[test]
fn lifecycle_success_preserves_the_task_3_2_limited_abi_claim() {
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
    let table = synthetic_lifecycle_api_table();
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
    let api = runtime
        .lifecycle_api_with_main_thread_check(synthetic_main_thread_check)
        .unwrap();
    let result = api.exec(AppCommand::LoopIteration, 0, 0).unwrap();

    assert_eq!(
        result.validated_fields(),
        &[AbiField::ApiVersion, AbiField::SciterVersion]
    );
    assert!(!result.validates_lifecycle_api());
}

#[test]
fn lifecycle_helpers_use_official_commands_and_keep_debug_context_stable_through_shutdown() {
    LIFECYCLE_COMMANDS.with(|commands| commands.borrow_mut().clear());
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
    let table = synthetic_lifecycle_api_table();
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
    let api = runtime
        .lifecycle_api_with_main_thread_check(synthetic_main_thread_check)
        .unwrap();
    let window = api.create_window(WindowFlags::MAIN, None, None).unwrap();
    let debug = DebugCallbackContext::new("MDLUMA_EVIDENCE\t");
    let address = debug.stable_address();
    let registered_debug = api.register_debug_output(None, debug).unwrap();

    assert_eq!(registered_debug.stable_address(), address);
    assert_eq!(registered_debug.protocol_prefix(), "MDLUMA_EVIDENCE\t");
    assert_eq!(registered_debug.callback_count().unwrap(), 1);
    assert_eq!(
        api.load_html(window, b"<html></html>", None).unwrap().raw(),
        1
    );
    assert_eq!(
        api.set_window_state(window, WindowState::Shown, false)
            .unwrap()
            .raw(),
        1
    );
    assert_eq!(api.window_state(window).unwrap(), WindowState::Shown);
    api.init().unwrap();
    assert!(api.loop_iteration().unwrap());
    api.stop().unwrap();
    let shutdown = api.shutdown().unwrap();
    registered_debug.release_after_shutdown(shutdown);

    LIFECYCLE_COMMANDS.with(|commands| assert_eq!(*commands.borrow(), [2, 6, 0, 3]));
}

#[test]
fn lifecycle_helpers_match_the_pinned_official_wrapper_return_contracts() {
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
    let table = synthetic_lifecycle_api_table();
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
    let api = runtime
        .lifecycle_api_with_main_thread_check(synthetic_main_thread_check)
        .unwrap();

    LIFECYCLE_EXEC_RESULT.with(|result| *result.borrow_mut() = Some((AppCommand::Init as u32, 17)));
    api.init().unwrap();

    LIFECYCLE_EXEC_RESULT.with(|result| *result.borrow_mut() = Some((AppCommand::Stop as u32, 17)));
    assert_eq!(
        api.stop(),
        Err(LifecycleError::CommandRejected {
            command: AppCommand::Stop,
            raw: 17,
        })
    );

    LIFECYCLE_EXEC_RESULT
        .with(|result| *result.borrow_mut() = Some((AppCommand::Shutdown as u32, 17)));
    api.shutdown().unwrap();
    LIFECYCLE_EXEC_RESULT.with(|result| *result.borrow_mut() = None);
}

#[test]
fn off_main_host_callback_records_typed_failure_for_the_owner() {
    reset_context_drop_counts_for_tests();
    SYNTHETIC_MAIN_THREAD.with(|is_main| *is_main.borrow_mut() = true);
    let mut table = synthetic_lifecycle_api_table();
    table.SciterSetCallback = Some(synthetic_set_callback_without_notification);
    let runtime = unsafe { SciterRuntime::from_api_table_for_tests(&table) };
    let api = runtime
        .lifecycle_api_with_main_thread_check(synthetic_main_thread_check)
        .unwrap();
    let window = api.create_window(WindowFlags::MAIN, None, None).unwrap();
    let mut registered = api
        .register_host_callback(window, HostCallbackContext::new())
        .unwrap();

    registered.invoke_destroy_off_main_for_tests();

    assert_eq!(
        registered.destroyed(),
        Err(LifecycleError::HostCallback(
            HostCallbackFailure::OffMainThread
        ))
    );
    drop(registered);
    assert_eq!(host_context_drop_count_for_tests(), 0);
}

#[test]
fn debug_callback_uses_the_exact_utf16_pointer_length_without_lossy_conversion() {
    let received = std::rc::Rc::new(RefCell::new(Vec::<String>::new()));
    let captured = std::rc::Rc::clone(&received);
    let mut context = DebugCallbackContext::with_handler("MDLUMA_EVIDENCE", move |text| {
        captured.borrow_mut().push(text.to_owned());
        Ok(())
    });
    let text: Vec<_> = "MDLUMA_EVIDENCE\t1\tpopup\t1\tstartedTRAILING"
        .encode_utf16()
        .collect();
    let exact_length = text.len() - "TRAILING".len();

    unsafe { context.invoke_for_tests(text.as_ptr(), exact_length as u32) };

    assert_eq!(
        received.borrow().as_slice(),
        &["MDLUMA_EVIDENCE\t1\tpopup\t1\tstarted"]
    );
    assert_eq!(context.failure_for_tests(), None);
}

#[test]
fn debug_callback_rejects_null_unaligned_and_invalid_utf16_inputs() {
    let invalid_utf16 = [0xd800];
    let cases = [
        (std::ptr::null(), 1, "null UTF-16 pointer"),
        (1usize as *const u16, 1, "unaligned UTF-16 pointer"),
        (invalid_utf16.as_ptr(), 1, "not valid UTF-16"),
    ];

    for (pointer, length, expected) in cases {
        let mut context = DebugCallbackContext::new("MDLUMA_EVIDENCE");
        unsafe { context.invoke_for_tests(pointer, length) };
        assert!(
            context.failure_for_tests().unwrap().contains(expected),
            "missing {expected}"
        );
    }
}

#[test]
fn popup_fixture_has_a_deterministic_100_cycle_asset_contract() {
    let fixture_dir = toolkit_dir().join("fixtures");
    let html = fs::read_to_string(fixture_dir.join("popup.htm")).unwrap();
    let state_machine = fs::read_to_string(fixture_dir.join("popup-state.js")).unwrap();

    for required in [
        "<button #anchor",
        "<menu #popup-menu",
        "anchor.popup(menu,{anchorAt:1,popupAt:7})",
        "popup.isValid === true",
        "popup.on(\"close\"",
        "anchor.post(function closePopup()",
        "popup.close()",
        "anchor.post(function confirmClosed()",
        "popup.isValid === false",
        "Window.this.close()",
    ] {
        assert!(
            html.contains(required),
            "missing popup contract: {required}"
        );
    }
    for forbidden in [
        "setTimeout",
        "setInterval",
        "deadline",
        "gate=",
        "decision=",
        "SCITER_APP_",
        "SC_ENGINE_",
    ] {
        assert!(
            !html.contains(forbidden) && !state_machine.contains(forbidden),
            "fixture owns forbidden responsibility: {forbidden}"
        );
    }

    let node_program = r#"
const reducer = require(process.argv[1]);
let state = reducer.initialState();
const events = [];
for (;;) {
  let transition = reducer.transition(state, "begin");
  state = transition.state;
  events.push(transition.event);
  transition = reducer.transition(state, "shown");
  state = transition.state;
  events.push(transition.event);
  transition = reducer.transition(state, "closed");
  state = transition.state;
  events.push(transition.event);
  if (transition.action === "close-controller") break;
}
if (events.length !== 300 || state.cycle !== 100 || state.phase !== "complete") process.exit(1);
for (let cycle = 1; cycle <= 100; cycle += 1) {
  const offset = (cycle - 1) * 3;
  const expected = ["started", "shown", "closed"];
  for (let phase = 0; phase < 3; phase += 1) {
    if (events[offset + phase].cycle !== cycle || events[offset + phase].phase !== expected[phase]) process.exit(2);
  }
}
"#;
    let output = Command::new("node")
        .args(["-e", node_program])
        .arg(fixture_dir.join("popup-state.js"))
        .output()
        .expect("Node is required to execute the pure popup transition reducer");
    assert!(
        output.status.success(),
        "pure popup reducer contract failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires the pinned Sciter dylib on a native arm64 macOS host"]
fn pinned_sciter_runtime_abi_smoke() {
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

    let helper_dir = TestDirectory::new("pinned-abi-main-thread");
    let helper_source = helper_dir.path.join("smoke.rs");
    let helper_binary = helper_dir.path.join("smoke");
    let model_path = toolkit_dir().join("model.rs");
    let manifest_path = toolkit_dir().join("manifest.rs");
    let sciter_path = toolkit_dir().join("sciter.rs");
    let artifact_manifest_path = repository_root
        .join(".kiro/specs/macos-sciter-runtime-evidence/evidence/artifact-manifest.txt");
    let harness_path = toolkit_dir().join("harness.rs");
    let source = format!(
        r#"
#[path = {model_path:?}]
mod model;
#[path = {manifest_path:?}]
mod manifest;
#[path = {sciter_path:?}]
mod sciter;
#[path = {harness_path:?}]
mod harness;

fn main() {{
    let manifest_text = std::fs::read_to_string({artifact_manifest_path:?}).unwrap();
    let manifest = manifest::parse_artifact_manifest(&manifest_text).unwrap();
    let runtime = std::path::Path::new({runtime:?}).canonicalize().unwrap();
    harness::run_child(
        harness::ChildMode::ApiAbi,
        &runtime,
        std::path::Path::new("unused-for-api-abi"),
        &manifest,
    ).unwrap();
}}
"#,
    );
    fs::write(&helper_source, source).unwrap();

    compile_rust_source(&helper_source, &helper_binary);
    let architectures = Command::new("/usr/bin/lipo")
        .args(["-archs"])
        .arg(&helper_binary)
        .output()
        .unwrap();
    assert!(architectures.status.success());
    assert_eq!(
        String::from_utf8(architectures.stdout).unwrap().trim(),
        "arm64"
    );

    let smoke = Command::new(helper_binary).output().unwrap();
    assert!(
        smoke.status.success(),
        "pinned ABI smoke failed: stdout={} stderr={}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert!(smoke.stderr.is_empty());
    let protocol = parse_child_protocol(&smoke.stdout).unwrap();
    let api = protocol.iter().find(|line| {
        line.record == "progress"
            && line.fields["stage"] == "api_version"
            && line.fields["state"] == "completed"
    });
    assert_eq!(api.unwrap().fields["actual"], "10");
    let engine = protocol.iter().find(|line| {
        line.record == "progress"
            && line.fields["stage"] == "sciter_version_call"
            && line.fields["state"] == "completed"
    });
    assert_eq!(engine.unwrap().fields["actual"], "6.0.3.18");
    let result = protocol.last().unwrap();
    assert_eq!(result.fields["status"], "success_candidate");
    assert_eq!(result.fields["scope"], "api_version+SciterVersion");
    assert_eq!(result.fields["lifecycle_abi_validated"], "false");
    assert_eq!(result.fields["process_architecture"], "arm64");
    assert_eq!(result.fields["thread_context"], "main");
    assert!(!String::from_utf8(smoke.stdout)
        .unwrap()
        .contains("gate=pass"));
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
