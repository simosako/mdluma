use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use crate::model::{ArtifactManifest, EvidenceError, LicenseEvidence, PermissionStatus};

const REQUIRED_KEYS: [&str; 12] = [
    "schema_version",
    "repository",
    "commit",
    "sdk_relative_path",
    "workspace_relative_path",
    "sha256",
    "engine_version",
    "api_version",
    "version_header_path",
    "api_header_path",
    "version_header_source",
    "api_header_source",
];

const REPOSITORY: &str = "https://gitlab.com/sciter-engine/sciter-js-sdk";
const COMMIT: &str = "e31ec0f726bdbe5d0402ad647f3b34feef84654e";
const SDK_RELATIVE_PATH: &str = "bin/macosx/libsciter.dylib";
const WORKSPACE_RELATIVE_PATH: &str = "vendor/sciter-js-sdk-main/bin/macosx/libsciter.dylib";
const SHA256: &str = "be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b";
const ENGINE_VERSION: [u32; 4] = [6, 0, 3, 18];
const API_VERSION: u32 = 10;
const VERSION_HEADER_PATH: &str = "vendor/sciter-js-sdk-main/include/sciter-version.h";
const API_HEADER_PATH: &str = "vendor/sciter-js-sdk-main/include/sciter-x-api.h";
const VERSION_HEADER_SOURCE: &str = "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-version.h";
const API_HEADER_SOURCE: &str = "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-x-api.h";
const BASELINE_FIELDS: [(&str, &str); 12] = [
    ("schema_version", "1"),
    ("repository", REPOSITORY),
    ("commit", COMMIT),
    ("sdk_relative_path", SDK_RELATIVE_PATH),
    ("workspace_relative_path", WORKSPACE_RELATIVE_PATH),
    ("sha256", SHA256),
    ("engine_version", "6.0.3.18"),
    ("api_version", "10"),
    ("version_header_path", VERSION_HEADER_PATH),
    ("api_header_path", API_HEADER_PATH),
    ("version_header_source", VERSION_HEADER_SOURCE),
    ("api_header_source", API_HEADER_SOURCE),
];
const LICENSE_REQUIRED_KEYS: [&str; 8] = [
    "schema_version",
    "redistribution",
    "resigning",
    "license_source",
    "eula_source",
    "permission_source",
    "required_about_text",
    "required_distribution_files",
];
const LICENSE_SOURCE: &str = "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/LICENSE";
const EULA_SOURCE: &str = "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/SCITER-ENGINE-EULA.md";
const FIXED_REVISION_SOURCE_PREFIX: &str = "https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/";
const SCITER_SOURCE_PREFIX: &str = "https://sciter.com/";
const REQUIRED_ABOUT_TEXT: &str = "This application uses Sciter Engine (http://sciter.com/), copyright Terra Informatica Software, Inc.";
const REQUIRED_DISTRIBUTION_FILES: &str = "LICENSE,SCITER-ENGINE-EULA.md";

pub(crate) fn parse_artifact_manifest(input: &str) -> Result<ArtifactManifest, EvidenceError> {
    let mut fields = BTreeMap::new();
    for (index, line) in input.lines().enumerate() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid(format!("line {} is not a key=value entry", index + 1)))?;
        if key.is_empty() || value.is_empty() {
            return Err(invalid(format!(
                "line {} has an empty key or value",
                index + 1
            )));
        }
        if !REQUIRED_KEYS.contains(&key) {
            return Err(invalid(format!("unknown key: {key}")));
        }
        if fields.insert(key, value).is_some() {
            return Err(invalid(format!("duplicate key: {key}")));
        }
    }

    for key in REQUIRED_KEYS {
        if !fields.contains_key(key) {
            return Err(invalid(format!("missing key: {key}")));
        }
    }

    let schema_version = parse_u32(field(&fields, "schema_version")?, "schema_version")?;
    let repository = field(&fields, "repository")?;
    let commit = field(&fields, "commit")?;
    validate_lower_hex(commit, 40, "commit")?;
    let sdk_relative_path =
        parse_relative_path(field(&fields, "sdk_relative_path")?, "sdk_relative_path")?;
    let workspace_relative_path = parse_relative_path(
        field(&fields, "workspace_relative_path")?,
        "workspace_relative_path",
    )?;
    let sha256 = field(&fields, "sha256")?;
    validate_lower_hex(sha256, 64, "sha256")?;
    let engine_version = parse_engine_version(field(&fields, "engine_version")?)?;
    let api_version = parse_u32(field(&fields, "api_version")?, "api_version")?;
    let version_header_path = parse_relative_path(
        field(&fields, "version_header_path")?,
        "version_header_path",
    )?;
    let api_header_path =
        parse_relative_path(field(&fields, "api_header_path")?, "api_header_path")?;
    let version_header_source = field(&fields, "version_header_source")?;
    let api_header_source = field(&fields, "api_header_source")?;

    for (key, expected) in BASELINE_FIELDS {
        require_baseline(key, field(&fields, key)?, expected)?;
    }

    require_baseline("schema_version", schema_version, 1)?;
    require_baseline("repository", repository, REPOSITORY)?;
    require_baseline("commit", commit, COMMIT)?;
    require_baseline(
        "sdk_relative_path",
        sdk_relative_path.as_path(),
        Path::new(SDK_RELATIVE_PATH),
    )?;
    require_baseline(
        "workspace_relative_path",
        workspace_relative_path.as_path(),
        Path::new(WORKSPACE_RELATIVE_PATH),
    )?;
    require_baseline("sha256", sha256, SHA256)?;
    require_baseline("engine_version", engine_version, ENGINE_VERSION)?;
    require_baseline("api_version", api_version, API_VERSION)?;
    require_baseline(
        "version_header_path",
        version_header_path.as_path(),
        Path::new(VERSION_HEADER_PATH),
    )?;
    require_baseline(
        "api_header_path",
        api_header_path.as_path(),
        Path::new(API_HEADER_PATH),
    )?;
    require_baseline(
        "version_header_source",
        version_header_source,
        VERSION_HEADER_SOURCE,
    )?;
    require_baseline("api_header_source", api_header_source, API_HEADER_SOURCE)?;

    Ok(ArtifactManifest {
        schema_version,
        repository: repository.to_owned(),
        commit: commit.to_owned(),
        sdk_relative_path,
        workspace_relative_path,
        sha256: sha256.to_owned(),
        engine_version,
        api_version,
        version_header_path,
        api_header_path,
        version_header_source: version_header_source.to_owned(),
        api_header_source: api_header_source.to_owned(),
    })
}

pub(crate) fn parse_license_evidence(input: &str) -> Result<LicenseEvidence, EvidenceError> {
    let mut fields = BTreeMap::new();
    for (index, line) in input.lines().enumerate() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid(format!("line {} is not a key=value entry", index + 1)))?;
        if key.is_empty() || value.is_empty() || value.trim() != value {
            return Err(invalid(format!(
                "line {} has an empty or noncanonical key or value",
                index + 1
            )));
        }
        if value.chars().any(char::is_control) {
            return Err(invalid(format!(
                "line {} contains a control character",
                index + 1
            )));
        }
        if !LICENSE_REQUIRED_KEYS.contains(&key) {
            return Err(invalid(format!("unknown key: {key}")));
        }
        if fields.insert(key, value).is_some() {
            return Err(invalid(format!("duplicate key: {key}")));
        }
    }

    for key in LICENSE_REQUIRED_KEYS {
        if !fields.contains_key(key) {
            return Err(invalid(format!("missing key: {key}")));
        }
    }

    require_baseline("schema_version", field(&fields, "schema_version")?, "1")?;
    let schema_version = 1;
    let redistribution = parse_permission_status(field(&fields, "redistribution")?)?;
    let resigning = parse_permission_status(field(&fields, "resigning")?)?;
    let license_source = field(&fields, "license_source")?;
    let eula_source = field(&fields, "eula_source")?;
    require_baseline("license_source", license_source, LICENSE_SOURCE)?;
    require_baseline("eula_source", eula_source, EULA_SOURCE)?;
    let permission_source = field(&fields, "permission_source")?;
    validate_permission_source(permission_source)?;
    let required_about_text = field(&fields, "required_about_text")?;
    require_baseline(
        "required_about_text",
        required_about_text,
        REQUIRED_ABOUT_TEXT,
    )?;
    let distribution_files = field(&fields, "required_distribution_files")?;
    require_baseline(
        "required_distribution_files",
        distribution_files,
        REQUIRED_DISTRIBUTION_FILES,
    )?;
    let required_distribution_files = parse_distribution_files(distribution_files)?;

    Ok(LicenseEvidence {
        schema_version,
        redistribution,
        resigning,
        license_source: license_source.to_owned(),
        eula_source: eula_source.to_owned(),
        permission_source: permission_source.to_owned(),
        required_about_text: required_about_text.to_owned(),
        required_distribution_files,
    })
}

fn field<'a>(fields: &BTreeMap<&'a str, &'a str>, key: &str) -> Result<&'a str, EvidenceError> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| invalid(format!("missing key: {key}")))
}

fn parse_u32(value: &str, key: &str) -> Result<u32, EvidenceError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid(format!("invalid {key}: {value}")));
    }
    value
        .parse()
        .map_err(|_| invalid(format!("invalid {key}: {value}")))
}

fn parse_engine_version(value: &str) -> Result<[u32; 4], EvidenceError> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 4 {
        return Err(invalid(format!("invalid engine_version: {value}")));
    }
    let mut version = [0; 4];
    for (index, part) in parts.into_iter().enumerate() {
        version[index] = parse_u32(part, "engine_version")?;
    }
    Ok(version)
}

fn parse_permission_status(value: &str) -> Result<PermissionStatus, EvidenceError> {
    match value {
        "permitted" => Ok(PermissionStatus::Permitted),
        "prohibited" => Ok(PermissionStatus::Prohibited),
        "unresolved" => Ok(PermissionStatus::Unresolved),
        _ => Err(invalid(format!("invalid permission status: {value}"))),
    }
}

fn validate_permission_source(value: &str) -> Result<(), EvidenceError> {
    let official_path = value
        .strip_prefix(FIXED_REVISION_SOURCE_PREFIX)
        .or_else(|| value.strip_prefix(SCITER_SOURCE_PREFIX));
    if official_path.is_some_and(|path| {
        !path.is_empty() && parse_relative_path(path, "permission_source").is_ok()
    }) {
        return Ok(());
    }

    let path = parse_relative_path(value, "permission_source")?;
    let redacted_response = path.starts_with("provider-responses")
        && path.components().count() > 1
        && matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("md" | "txt")
        );
    if !redacted_response {
        return Err(invalid(format!("invalid permission_source: {value}")));
    }
    Ok(())
}

fn parse_distribution_files(value: &str) -> Result<Vec<String>, EvidenceError> {
    let mut files = Vec::new();
    for entry in value.split(',') {
        if entry.is_empty() || entry.trim() != entry {
            return Err(invalid(format!(
                "invalid required_distribution_files: {value}"
            )));
        }
        parse_relative_path(entry, "required_distribution_files")?;
        if files.iter().any(|file| file == entry) {
            return Err(invalid(format!("duplicate distribution file: {entry}")));
        }
        files.push(entry.to_owned());
    }
    Ok(files)
}

fn parse_relative_path(value: &str, key: &str) -> Result<PathBuf, EvidenceError> {
    let path = Path::new(value);
    let valid = !value.contains('\\')
        && !value.contains(':')
        && !value.split('/').any(str::is_empty)
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !valid {
        return Err(invalid(format!("invalid {key}: {value}")));
    }
    Ok(path.to_path_buf())
}

fn validate_lower_hex(value: &str, length: usize, key: &str) -> Result<(), EvidenceError> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(format!("invalid {key}: {value}")));
    }
    Ok(())
}

fn require_baseline<T>(key: &str, actual: T, expected: T) -> Result<(), EvidenceError>
where
    T: PartialEq,
{
    if actual != expected {
        return Err(invalid(format!("{key} does not match the fixed baseline")));
    }
    Ok(())
}

fn invalid(message: String) -> EvidenceError {
    EvidenceError::InvalidManifest(message)
}
