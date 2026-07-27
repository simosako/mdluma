use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{
    validate_criterion_results, CriterionId, CriterionResult, CriterionStatus, CycleKind,
    CyclePhase, DecisionState, EvidenceError, GateId, GateResult, GateStatus, HarnessEvent, RunId,
    ALL_CRITERIA, ALL_GATES, SOURCE_CRITERIA,
};

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
