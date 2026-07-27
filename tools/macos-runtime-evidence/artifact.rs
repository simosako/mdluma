use std::ffi::OsString;
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::{
    ArtifactManifest, CriterionId, CriterionResult, CriterionStatus, GateId, GateResult,
    GateStatus, NamedArtifact,
};

const OFFICIAL_REPOSITORY: &str = "https://gitlab.com/sciter-engine/sciter-js-sdk";
const OFFICIAL_COMMIT: &str = "e31ec0f726bdbe5d0402ad647f3b34feef84654e";
const OFFICIAL_SDK_PATH: &str = "bin/macosx/libsciter.dylib";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommandStatus {
    Exited(i32),
    Signaled(Option<i32>),
    NotStarted(String),
}

impl CommandStatus {
    fn succeeded(&self) -> bool {
        matches!(self, Self::Exited(0))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandCapture {
    pub(crate) program: String,
    pub(crate) arguments: Vec<OsString>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) status: CommandStatus,
}

pub(crate) trait CommandRunner {
    fn run(&self, program: &str, arguments: &[OsString]) -> CommandCapture;
}

pub(crate) struct SystemCommandRunner;

impl SystemCommandRunner {
    pub(crate) fn program_path(identifier: &str) -> Option<&'static str> {
        match identifier {
            "shasum" => Some("/usr/bin/shasum"),
            "lipo" => Some("/usr/bin/lipo"),
            "otool" => Some("/usr/bin/otool"),
            "codesign" => Some("/usr/bin/codesign"),
            _ => None,
        }
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, identifier: &str, arguments: &[OsString]) -> CommandCapture {
        let Some(program) = Self::program_path(identifier) else {
            return CommandCapture {
                program: identifier.to_owned(),
                arguments: arguments.to_vec(),
                stdout: Vec::new(),
                stderr: Vec::new(),
                status: CommandStatus::NotStarted(format!(
                    "command identifier is not allowed: {identifier}"
                )),
            };
        };
        match Command::new(program).args(arguments).output() {
            Ok(output) => CommandCapture {
                program: program.to_owned(),
                arguments: arguments.to_vec(),
                stdout: output.stdout,
                stderr: output.stderr,
                status: output.status.code().map_or_else(
                    || CommandStatus::Signaled(output.status.signal()),
                    CommandStatus::Exited,
                ),
            },
            Err(error) => CommandCapture {
                program: program.to_owned(),
                arguments: arguments.to_vec(),
                stdout: Vec::new(),
                stderr: Vec::new(),
                status: CommandStatus::NotStarted(error.to_string()),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CodeSigningState {
    AdHoc,
    Signed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProbeBundle {
    pub(crate) repository_root: Option<PathBuf>,
    pub(crate) workspace_runtime_path: Option<PathBuf>,
    pub(crate) runtime_path: Option<PathBuf>,
    pub(crate) provenance_repository: String,
    pub(crate) provenance_commit: String,
    pub(crate) provenance_sdk_path: PathBuf,
    pub(crate) expected_sha256: String,
    pub(crate) actual_sha256: Option<String>,
    pub(crate) hash_matches: bool,
    pub(crate) architectures: Vec<String>,
    pub(crate) minimum_macos_versions: Vec<(String, String)>,
    pub(crate) dependencies: Vec<String>,
    pub(crate) install_name: Option<String>,
    pub(crate) codesign_state: Option<CodeSigningState>,
    pub(crate) criteria: Vec<CriterionResult>,
    pub(crate) gates: Vec<GateResult>,
    pub(crate) raw_artifacts: Vec<NamedArtifact>,
    pub(crate) command_captures: Vec<CommandCapture>,
}

pub(crate) fn probe_artifact(
    manifest: &ArtifactManifest,
    repository_root: &Path,
    runtime_path: &Path,
) -> ProbeBundle {
    probe_artifact_with_runner(
        manifest,
        repository_root,
        runtime_path,
        &SystemCommandRunner,
    )
}

pub(crate) fn probe_artifact_with_runner<R: CommandRunner>(
    manifest: &ArtifactManifest,
    repository_root: &Path,
    runtime_path: &Path,
    runner: &R,
) -> ProbeBundle {
    let provenance_valid = manifest.repository == OFFICIAL_REPOSITORY
        && manifest.commit == OFFICIAL_COMMIT
        && manifest.sdk_relative_path == Path::new(OFFICIAL_SDK_PATH);
    let canonical_root = fs::canonicalize(repository_root).ok();
    let canonical_workspace = canonical_root
        .as_ref()
        .and_then(|root| fs::canonicalize(root.join(&manifest.workspace_relative_path)).ok());
    let canonical_runtime = fs::canonicalize(runtime_path).ok();
    let path_matches = canonical_workspace.is_some() && canonical_workspace == canonical_runtime;

    let mut bundle = ProbeBundle {
        repository_root: canonical_root,
        workspace_runtime_path: canonical_workspace,
        runtime_path: canonical_runtime,
        provenance_repository: manifest.repository.clone(),
        provenance_commit: manifest.commit.clone(),
        provenance_sdk_path: manifest.sdk_relative_path.clone(),
        expected_sha256: manifest.sha256.clone(),
        actual_sha256: None,
        hash_matches: false,
        architectures: Vec::new(),
        minimum_macos_versions: Vec::new(),
        dependencies: Vec::new(),
        install_name: None,
        codesign_state: None,
        criteria: Vec::new(),
        gates: Vec::new(),
        raw_artifacts: Vec::new(),
        command_captures: Vec::new(),
    };

    if !provenance_valid || !path_matches {
        populate_precondition_failure(&mut bundle, provenance_valid, path_matches);
        return bundle;
    }

    let runtime = bundle
        .runtime_path
        .as_ref()
        .expect("matching canonical runtime path")
        .as_os_str()
        .to_owned();
    let commands = [
        ("shasum", vec!["-a".into(), "256".into(), runtime.clone()]),
        ("lipo", vec!["-archs".into(), runtime.clone()]),
        ("otool", vec!["-l".into(), runtime.clone()]),
        ("otool", vec!["-L".into(), runtime.clone()]),
        ("otool", vec!["-D".into(), runtime.clone()]),
        (
            "codesign",
            vec!["-dv".into(), "--verbose=4".into(), runtime],
        ),
    ];

    for (program, arguments) in commands {
        bundle
            .command_captures
            .push(runner.run(program, &arguments));
    }
    bundle.raw_artifacts = raw_artifacts(&bundle.command_captures);

    let parsed_hash = successful_text(&bundle.command_captures[0]).and_then(parse_sha256);
    bundle.actual_sha256 = parsed_hash;
    bundle.hash_matches = bundle.actual_sha256.as_deref() == Some(manifest.sha256.as_str());
    bundle.architectures = successful_text(&bundle.command_captures[1])
        .map(parse_architectures)
        .unwrap_or_default();
    bundle.minimum_macos_versions = successful_text(&bundle.command_captures[2])
        .map(parse_minimum_versions)
        .unwrap_or_default();
    bundle.dependencies = successful_text(&bundle.command_captures[3])
        .map(parse_dependencies)
        .unwrap_or_default();
    bundle.install_name = successful_text(&bundle.command_captures[4]).and_then(parse_install_name);
    bundle.codesign_state = successful_combined_text(&bundle.command_captures[5])
        .and_then(|output| parse_codesign_state(&output));

    populate_results(&mut bundle);
    bundle
}

fn successful_text(capture: &CommandCapture) -> Option<&str> {
    capture
        .status
        .succeeded()
        .then(|| std::str::from_utf8(&capture.stdout).ok())
        .flatten()
}

fn successful_combined_text(capture: &CommandCapture) -> Option<String> {
    if !capture.status.succeeded() {
        return None;
    }
    let stdout = std::str::from_utf8(&capture.stdout).ok()?;
    let stderr = std::str::from_utf8(&capture.stderr).ok()?;
    Some(format!("{stdout}{stderr}"))
}

fn parse_sha256(output: &str) -> Option<String> {
    let hash = output.split_whitespace().next()?;
    (hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then(|| hash.to_owned())
}

fn parse_architectures(output: &str) -> Vec<String> {
    unique(output.split_whitespace().map(str::to_owned))
}

pub(crate) fn parse_minimum_versions(output: &str) -> Vec<(String, String)> {
    let mut architecture = None;
    let mut version_field = None;
    let mut versions = Vec::new();
    for line in output.lines() {
        if let Some(value) = architecture_heading(line) {
            architecture = Some(value.to_owned());
            version_field = None;
            continue;
        }
        let line = line.trim();
        if line == "cmd LC_BUILD_VERSION" {
            version_field = Some("minos ");
            continue;
        }
        if line == "cmd LC_VERSION_MIN_MACOSX" {
            version_field = Some("version ");
            continue;
        }
        if line.starts_with("cmd ") {
            version_field = None;
            continue;
        }
        let version = version_field.and_then(|field| line.strip_prefix(field));
        if let (Some(architecture), Some(version)) = (&architecture, version) {
            let entry = (architecture.clone(), version.to_owned());
            if !versions.contains(&entry) {
                versions.push(entry);
            }
            version_field = None;
        }
    }
    versions
}

fn architecture_heading(line: &str) -> Option<&str> {
    line.strip_suffix("):")?
        .rsplit_once(" (architecture ")
        .map(|(_, architecture)| architecture)
}

fn parse_dependencies(output: &str) -> Vec<String> {
    let entries = output.lines().filter_map(|line| {
        let line = line.strip_prefix('\t')?;
        line.split_once(" (compatibility version")
            .map(|(path, _)| path.to_owned())
    });
    let all = unique(entries);
    all.into_iter().skip(1).collect()
}

fn parse_install_name(output: &str) -> Option<String> {
    let names = unique(
        output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && architecture_heading(line).is_none())
            .map(str::to_owned),
    );
    (names.len() == 1).then(|| names[0].clone())
}

fn parse_codesign_state(output: &str) -> Option<CodeSigningState> {
    let signature = output
        .lines()
        .find_map(|line| line.strip_prefix("Signature="))?;
    Some(if signature == "adhoc" {
        CodeSigningState::AdHoc
    } else {
        CodeSigningState::Signed
    })
}

fn unique<T: Eq>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

fn raw_artifacts(captures: &[CommandCapture]) -> Vec<NamedArtifact> {
    let names = [
        "shasum",
        "lipo",
        "otool-load",
        "otool-dependencies",
        "otool-install-name",
        "codesign",
    ];
    let mut artifacts = Vec::with_capacity(captures.len() * 3);
    for (name, capture) in names.into_iter().zip(captures) {
        artifacts.push(named(
            format!("metadata/{name}.stdout"),
            capture.stdout.clone(),
        ));
        artifacts.push(named(
            format!("metadata/{name}.stderr"),
            capture.stderr.clone(),
        ));
        artifacts.push(named(
            format!("metadata/{name}.status"),
            format!("{:?}\n", capture.status).into_bytes(),
        ));
    }
    artifacts
}

fn named(path: String, bytes: Vec<u8>) -> NamedArtifact {
    NamedArtifact {
        relative_path: PathBuf::from(path),
        bytes,
    }
}

fn populate_precondition_failure(
    bundle: &mut ProbeBundle,
    provenance_valid: bool,
    path_matches: bool,
) {
    for criterion in 1..=4 {
        bundle.criteria.push(result(
            1,
            criterion,
            if provenance_valid {
                CriterionStatus::Satisfied
            } else {
                CriterionStatus::Unsatisfied
            },
            "official artifact provenance",
        ));
    }
    bundle.criteria.push(result(
        1,
        5,
        CriterionStatus::NotRun,
        "hash command not run",
    ));
    bundle.criteria.push(result(
        1,
        6,
        CriterionStatus::NotRun,
        "hash comparison not run",
    ));
    bundle.criteria.push(result(
        1,
        7,
        if provenance_valid && path_matches {
            CriterionStatus::Satisfied
        } else {
            CriterionStatus::Unsatisfied
        },
        "provenance and canonical path",
    ));
    for criterion in [1, 2, 5, 6, 7, 8] {
        bundle.criteria.push(result(
            2,
            criterion,
            CriterionStatus::NotRun,
            "metadata command not run",
        ));
    }
    bundle.criteria.push(result(
        7,
        5,
        if path_matches {
            CriterionStatus::Satisfied
        } else {
            CriterionStatus::Unsatisfied
        },
        "canonical runtime path",
    ));
    bundle.criteria.push(result(
        7,
        6,
        CriterionStatus::NotRun,
        "runtime hash not collected",
    ));
    bundle.gates.push(gate(GateId::Artifact, &bundle.criteria));
    bundle.gates.push(GateResult::new(
        GateId::Platform,
        GateStatus::NotRun,
        GateId::Platform.criteria().to_vec(),
        "platform gate requires host process checks from Task 2.2",
    ));
}

fn populate_results(bundle: &mut ProbeBundle) {
    for criterion in 1..=4 {
        bundle.criteria.push(result(
            1,
            criterion,
            CriterionStatus::Satisfied,
            "fixed official provenance",
        ));
    }
    bundle.criteria.push(result(
        1,
        5,
        if bundle.actual_sha256.is_some() {
            CriterionStatus::Satisfied
        } else {
            CriterionStatus::NotRun
        },
        "actual SHA-256",
    ));
    bundle.criteria.push(result(
        1,
        6,
        if bundle.actual_sha256.is_none() {
            CriterionStatus::NotRun
        } else if bundle.hash_matches {
            CriterionStatus::Satisfied
        } else {
            CriterionStatus::Unsatisfied
        },
        "expected and actual SHA-256 comparison",
    ));
    bundle.criteria.push(result(
        1,
        7,
        CriterionStatus::Satisfied,
        "official provenance and canonical path",
    ));

    let architecture_status =
        if bundle.command_captures[1].status.succeeded() && !bundle.architectures.is_empty() {
            CriterionStatus::Satisfied
        } else {
            CriterionStatus::NotRun
        };
    bundle.criteria.push(result(
        2,
        1,
        architecture_status,
        "all Mach-O architectures",
    ));
    bundle.criteria.push(result(
        2,
        2,
        if architecture_status == CriterionStatus::NotRun {
            CriterionStatus::NotRun
        } else if bundle
            .architectures
            .iter()
            .any(|architecture| architecture == "arm64")
        {
            CriterionStatus::Satisfied
        } else {
            CriterionStatus::Unsatisfied
        },
        "arm64 architecture presence",
    ));
    for (criterion, present, summary) in [
        (
            5,
            !bundle.minimum_macos_versions.is_empty(),
            "minimum macOS versions",
        ),
        (
            6,
            !bundle.dependencies.is_empty(),
            "external dylib dependencies",
        ),
        (7, bundle.install_name.is_some(), "install name"),
        (8, bundle.codesign_state.is_some(), "code signing state"),
    ] {
        bundle.criteria.push(result(
            2,
            criterion,
            if present {
                CriterionStatus::Satisfied
            } else {
                CriterionStatus::NotRun
            },
            summary,
        ));
    }
    bundle.criteria.push(result(
        7,
        5,
        CriterionStatus::Satisfied,
        "canonical runtime path",
    ));
    bundle.criteria.push(result(
        7,
        6,
        if bundle.actual_sha256.is_some() {
            CriterionStatus::Satisfied
        } else {
            CriterionStatus::NotRun
        },
        "runtime SHA-256",
    ));
    bundle.gates.push(gate(GateId::Artifact, &bundle.criteria));
    bundle.gates.push(GateResult::new(
        GateId::Platform,
        GateStatus::NotRun,
        GateId::Platform.criteria().to_vec(),
        "platform gate requires host process checks from Task 2.2",
    ));
}

fn result(
    requirement: u8,
    criterion: u8,
    status: CriterionStatus,
    summary: &str,
) -> CriterionResult {
    CriterionResult::new(
        CriterionId::from_parts(requirement, criterion),
        status,
        summary,
        Vec::new(),
    )
}

fn gate(id: GateId, results: &[CriterionResult]) -> GateResult {
    gate_with_ids(id, results, id.criteria())
}

fn gate_with_ids(id: GateId, results: &[CriterionResult], ids: &[CriterionId]) -> GateResult {
    let statuses = ids.iter().filter_map(|id| {
        results
            .iter()
            .find(|result| result.id() == *id)
            .map(CriterionResult::status)
    });
    let statuses: Vec<_> = statuses.collect();
    let status = if statuses
        .iter()
        .any(|status| *status == CriterionStatus::Unsatisfied)
    {
        GateStatus::Fail
    } else if statuses.len() != ids.len()
        || statuses
            .iter()
            .any(|status| *status == CriterionStatus::NotRun)
    {
        GateStatus::NotRun
    } else {
        GateStatus::Pass
    };
    GateResult::new(id, status, ids.to_vec(), "artifact probe checks")
}
