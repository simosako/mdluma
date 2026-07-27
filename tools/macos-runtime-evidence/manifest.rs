use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use crate::model::{ArtifactManifest, EvidenceError};

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

fn parse_relative_path(value: &str, key: &str) -> Result<PathBuf, EvidenceError> {
    let path = Path::new(value);
    let valid = !value.contains('\\')
        && !value.contains(':')
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
