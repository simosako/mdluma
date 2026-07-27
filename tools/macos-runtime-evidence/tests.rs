use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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
