use std::ffi::OsString;
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{
    ArtifactManifest, CriterionId, CriterionResult, CriterionStatus, GateId, GateResult,
    GateStatus, HeaderEvidence, HostSnapshot, NamedArtifact,
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
            "sysctl" => Some("/usr/sbin/sysctl"),
            "sw_vers" => Some("/usr/bin/sw_vers"),
            "uname" => Some("/usr/bin/uname"),
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
    pub(crate) started_at_utc: String,
    pub(crate) host: HostSnapshot,
    pub(crate) header_evidence: HeaderProbeOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeaderProbeOutcome {
    Verified(HeaderEvidence),
    Disagreed {
        header_engine_version: [u32; 4],
        header_api_version: u32,
        bindings_engine_version: [u32; 4],
        bindings_api_version: u32,
    },
    NotRun {
        reason: String,
    },
}

pub(crate) fn probe_artifact(
    manifest: &ArtifactManifest,
    repository_root: &Path,
    runtime_path: &Path,
) -> ProbeBundle {
    probe_artifact_with_runner_at(
        manifest,
        repository_root,
        runtime_path,
        &SystemCommandRunner,
        SystemTime::now(),
    )
}

pub(crate) fn probe_artifact_with_runner_at<R: CommandRunner>(
    manifest: &ArtifactManifest,
    repository_root: &Path,
    runtime_path: &Path,
    runner: &R,
    started_at: SystemTime,
) -> ProbeBundle {
    let mut bundle = probe_artifact_with_runner(manifest, repository_root, runtime_path, runner);
    collect_host(&mut bundle, runner, started_at);
    collect_headers(&mut bundle, manifest, repository_root);
    bundle
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
        started_at_utc: String::new(),
        host: HostSnapshot::default(),
        header_evidence: HeaderProbeOutcome::NotRun {
            reason: "host and header probe not run".to_owned(),
        },
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

fn collect_host<R: CommandRunner>(bundle: &mut ProbeBundle, runner: &R, started_at: SystemTime) {
    let captures = [
        runner.run("sysctl", &["-n".into(), "hw.model".into()]),
        runner.run("sw_vers", &["-productVersion".into()]),
        runner.run("uname", &["-m".into()]),
    ];
    let names = ["sysctl-hardware", "sw-vers", "uname-architecture"];
    for (name, capture) in names.into_iter().zip(&captures) {
        append_capture_artifacts(&mut bundle.raw_artifacts, "metadata/host", name, capture);
    }
    bundle.command_captures.extend(captures);

    let host_start = bundle.command_captures.len() - 3;
    bundle.host.hardware = capture_single_line(&bundle.command_captures[host_start])
        .unwrap_or_default()
        .to_owned();
    bundle.host.macos_version = capture_single_line(&bundle.command_captures[host_start + 1])
        .unwrap_or_default()
        .to_owned();
    bundle.host.process_architecture =
        capture_single_line(&bundle.command_captures[host_start + 2])
            .unwrap_or_default()
            .to_owned();
    bundle.started_at_utc = format_utc(started_at).unwrap_or_default();
    bundle.raw_artifacts.push(named(
        "metadata/host/executed-at-utc.txt".to_owned(),
        format!("{}\n", bundle.started_at_utc).into_bytes(),
    ));

    upsert_result(
        &mut bundle.criteria,
        result(
            2,
            3,
            present_status(&bundle.host.process_architecture),
            "recorded process architecture",
        ),
    );
    upsert_result(
        &mut bundle.criteria,
        result(
            2,
            4,
            if bundle.host.process_architecture.is_empty() {
                CriterionStatus::NotRun
            } else if bundle.host.process_architecture == "arm64" {
                CriterionStatus::Satisfied
            } else {
                CriterionStatus::Unsatisfied
            },
            "native arm64 process architecture",
        ),
    );
    for (criterion, value, summary) in [
        (1, bundle.started_at_utc.as_str(), "UTC execution time"),
        (2, bundle.host.hardware.as_str(), "recorded hardware"),
        (
            3,
            bundle.host.macos_version.as_str(),
            "recorded macOS version",
        ),
        (
            4,
            bundle.host.process_architecture.as_str(),
            "recorded process architecture",
        ),
    ] {
        upsert_result(
            &mut bundle.criteria,
            result(7, criterion, present_status(value), summary),
        );
    }
    replace_gate(
        bundle,
        GateId::Platform,
        "artifact and host platform checks",
    );
}

fn collect_headers(bundle: &mut ProbeBundle, manifest: &ArtifactManifest, repository_root: &Path) {
    upsert_result(
        &mut bundle.criteria,
        result(
            3,
            1,
            if manifest.engine_version == [6, 0, 3, 18] {
                CriterionStatus::Satisfied
            } else {
                CriterionStatus::Unsatisfied
            },
            "expected engine version 6.0.3.18",
        ),
    );
    upsert_result(
        &mut bundle.criteria,
        result(
            3,
            2,
            if manifest.api_version == 10 {
                CriterionStatus::Satisfied
            } else {
                CriterionStatus::Unsatisfied
            },
            "expected API version 10",
        ),
    );

    let identity_result = validate_header_identity(manifest);
    let version_path = repository_root.join(&manifest.version_header_path);
    let api_path = repository_root.join(&manifest.api_header_path);
    let bindings_path = repository_root.join("src/sciter/generated_sciter_bindings.rs");
    let sources = [
        ("sciter-version.h", version_path),
        ("sciter-x-api.h", api_path),
        ("generated-sciter-bindings.rs", bindings_path),
    ];
    let mut bytes = Vec::new();
    let mut read_error = None;
    for (name, path) in sources {
        match fs::read(&path) {
            Ok(source) => {
                bundle
                    .raw_artifacts
                    .push(named(format!("metadata/headers/{name}"), source.clone()));
                bytes.push(source);
            }
            Err(error) => {
                read_error = Some(format!("{}: {error}", path.display()));
                break;
            }
        }
    }

    let parsed = identity_result.and_then(|()| {
        if let Some(reason) = read_error {
            return Err(reason);
        }
        let header_engine = parse_engine_defines(&bytes[0])?;
        let header_api = parse_c_define(&bytes[1], "SCITER_API_VERSION")?;
        let bindings_engine = parse_bindings_engine_constants(&bytes[2])?;
        let bindings_api = parse_rust_constant(&bytes[2], "SCITER_API_VERSION")?;
        Ok((header_engine, header_api, bindings_engine, bindings_api))
    });

    match parsed {
        Ok((header_engine, header_api, bindings_engine, bindings_api)) => {
            let engine_matches =
                header_engine == bindings_engine && header_engine == manifest.engine_version;
            let api_matches = header_api == bindings_api && header_api == manifest.api_version;
            if engine_matches && api_matches {
                let raw_artifacts = bundle
                    .raw_artifacts
                    .iter()
                    .filter(|artifact| artifact.relative_path.starts_with("metadata/headers"))
                    .cloned()
                    .collect();
                bundle.header_evidence = HeaderProbeOutcome::Verified(HeaderEvidence {
                    commit: manifest.commit.clone(),
                    engine_version: header_engine,
                    api_version: header_api,
                    raw_artifacts,
                });
                upsert_header_results(
                    bundle,
                    CriterionStatus::Satisfied,
                    CriterionStatus::Satisfied,
                    CriterionStatus::NotApplicable,
                );
            } else {
                bundle.header_evidence = HeaderProbeOutcome::Disagreed {
                    header_engine_version: header_engine,
                    header_api_version: header_api,
                    bindings_engine_version: bindings_engine,
                    bindings_api_version: bindings_api,
                };
                upsert_header_results(
                    bundle,
                    if engine_matches {
                        CriterionStatus::Satisfied
                    } else {
                        CriterionStatus::Unsatisfied
                    },
                    if api_matches {
                        CriterionStatus::Satisfied
                    } else {
                        CriterionStatus::Unsatisfied
                    },
                    CriterionStatus::Unsatisfied,
                );
            }
        }
        Err(reason) => {
            bundle.header_evidence = HeaderProbeOutcome::NotRun { reason };
            upsert_header_results(
                bundle,
                CriterionStatus::NotRun,
                CriterionStatus::NotRun,
                CriterionStatus::NotRun,
            );
        }
    }
    replace_gate(
        bundle,
        GateId::Api,
        "expected API baseline and runtime checks",
    );
    replace_gate(bundle, GateId::Abi, "same-revision header comparison");
}

fn validate_header_identity(manifest: &ArtifactManifest) -> Result<(), String> {
    let source_prefix = format!(
        "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/{}/",
        manifest.commit
    );
    let expected = [
        (
            manifest.version_header_path.as_path(),
            Path::new("vendor/sciter-js-sdk-main/include/sciter-version.h"),
            format!("{source_prefix}include/sciter-version.h"),
            manifest.version_header_source.as_str(),
        ),
        (
            manifest.api_header_path.as_path(),
            Path::new("vendor/sciter-js-sdk-main/include/sciter-x-api.h"),
            format!("{source_prefix}include/sciter-x-api.h"),
            manifest.api_header_source.as_str(),
        ),
    ];
    if expected
        .iter()
        .all(|(path, expected_path, source, actual)| path == expected_path && source == actual)
    {
        Ok(())
    } else {
        Err("manifest does not establish same-revision header identity".to_owned())
    }
}

fn parse_engine_defines(source: &[u8]) -> Result<[u32; 4], String> {
    Ok([
        parse_c_define(source, "SCITER_VERSION_0")?,
        parse_c_define(source, "SCITER_VERSION_1")?,
        parse_c_define(source, "SCITER_VERSION_2")?,
        parse_c_define(source, "SCITER_VERSION_3")?,
    ])
}

fn parse_bindings_engine_constants(source: &[u8]) -> Result<[u32; 4], String> {
    Ok([
        parse_rust_constant(source, "SCITER_VERSION_0")?,
        parse_rust_constant(source, "SCITER_VERSION_1")?,
        parse_rust_constant(source, "SCITER_VERSION_2")?,
        parse_rust_constant(source, "SCITER_VERSION_3")?,
    ])
}

fn parse_c_define(source: &[u8], name: &str) -> Result<u32, String> {
    let text = std::str::from_utf8(source).map_err(|error| error.to_string())?;
    let values: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.len() == 3 && fields[0] == "#define" && fields[1] == name).then_some(fields[2])
        })
        .collect();
    if values.len() != 1 || !values[0].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "expected one exact authoritative #define for {name}"
        ));
    }
    values[0]
        .parse()
        .map_err(|_| format!("invalid authoritative #define for {name}"))
}

fn parse_rust_constant(source: &[u8], name: &str) -> Result<u32, String> {
    let text = std::str::from_utf8(source).map_err(|error| error.to_string())?;
    let expected_name = format!("{name}:");
    let values: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.len() == 6
                && fields[0] == "pub"
                && fields[1] == "const"
                && fields[2] == expected_name
                && fields[3] == "u32"
                && fields[4] == "=")
                .then(|| fields[5].strip_suffix(';'))
                .flatten()
        })
        .collect();
    if values.len() != 1 || !values[0].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "expected one exact committed binding constant for {name}"
        ));
    }
    values[0]
        .parse()
        .map_err(|_| format!("invalid committed binding constant for {name}"))
}

fn capture_single_line(capture: &CommandCapture) -> Option<&str> {
    let text = successful_text(capture)?;
    let mut lines = text.lines();
    let value = lines.next()?;
    (!value.is_empty() && lines.next().is_none() && value.trim() == value).then_some(value)
}

fn present_status(value: &str) -> CriterionStatus {
    if value.is_empty() {
        CriterionStatus::NotRun
    } else {
        CriterionStatus::Satisfied
    }
}

fn upsert_header_results(
    bundle: &mut ProbeBundle,
    engine: CriterionStatus,
    api: CriterionStatus,
    disagreement: CriterionStatus,
) {
    for result in [
        result(
            4,
            1,
            engine,
            "headers, bindings, and manifest engine version",
        ),
        result(4, 2, api, "headers, bindings, and manifest API version"),
        result(
            4,
            7,
            disagreement,
            "header and binding constant disagreement",
        ),
    ] {
        upsert_result(&mut bundle.criteria, result);
    }
}

fn upsert_result(results: &mut Vec<CriterionResult>, replacement: CriterionResult) {
    if let Some(existing) = results
        .iter_mut()
        .find(|result| result.id() == replacement.id())
    {
        *existing = replacement;
    } else {
        results.push(replacement);
    }
}

fn replace_gate(bundle: &mut ProbeBundle, id: GateId, summary: &str) {
    let replacement = gate_with_ids(id, &bundle.criteria, id.criteria());
    let replacement = GateResult::new(id, replacement.status(), id.criteria().to_vec(), summary);
    if let Some(existing) = bundle.gates.iter_mut().find(|gate| gate.id() == id) {
        *existing = replacement;
    } else {
        bundle.gates.push(replacement);
    }
}

fn append_capture_artifacts(
    artifacts: &mut Vec<NamedArtifact>,
    directory: &str,
    name: &str,
    capture: &CommandCapture,
) {
    artifacts.push(named(
        format!("{directory}/{name}.stdout"),
        capture.stdout.clone(),
    ));
    artifacts.push(named(
        format!("{directory}/{name}.stderr"),
        capture.stderr.clone(),
    ));
    artifacts.push(named(
        format!("{directory}/{name}.status"),
        format!("{:?}\n", capture.status).into_bytes(),
    ));
}

fn format_utc(time: SystemTime) -> Option<String> {
    let seconds = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3_600,
        day_seconds % 3_600 / 60,
        day_seconds % 60
    ))
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let days = days_since_epoch + 719_468;
    let era = days / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
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
    bundle.gates.push(gate(GateId::Platform, &bundle.criteria));
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
