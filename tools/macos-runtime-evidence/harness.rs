use std::fmt;
use std::io::{self, Write};
use std::path::Path;

use crate::model::ArtifactManifest;
use crate::sciter::{
    AbiSmokeProgress, RuntimeAbiError, RuntimeLoadError, RuntimeLoadProgress, SciterRuntime,
};

const PROTOCOL_PREFIX: &str = "MDLUMA_EVIDENCE";
const PROTOCOL_VERSION: u16 = 1;
pub(crate) const LIMITED_ABI_SCOPE: &str = "api_version+SciterVersion";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildMode {
    ApiAbi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HarnessStage {
    RuntimeLoad,
    SciterApiExport,
    ApiTable,
    ApiVersion,
    SciterVersionCall,
    ProcessArchitecture,
    ThreadContext,
}

impl HarnessStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeLoad => "runtime_load",
            Self::SciterApiExport => "sciter_api_export",
            Self::ApiTable => "api_table",
            Self::ApiVersion => "api_version",
            Self::SciterVersionCall => "sciter_version_call",
            Self::ProcessArchitecture => "process_architecture",
            Self::ThreadContext => "thread_context",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProbeEvent {
    Entered(HarnessStage),
    Completed {
        stage: HarnessStage,
        actual: String,
        expected: String,
    },
}

impl ProbeEvent {
    pub(crate) const fn entered(stage: HarnessStage) -> Self {
        Self::Entered(stage)
    }

    pub(crate) fn completed(
        stage: HarnessStage,
        actual: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Self::Completed {
            stage,
            actual: actual.into(),
            expected: expected.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ApiAbiFailure {
    stage: HarnessStage,
    diagnostic: String,
}

impl ApiAbiFailure {
    pub(crate) fn new(stage: HarnessStage, diagnostic: impl Into<String>) -> Self {
        Self {
            stage,
            diagnostic: diagnostic.into(),
        }
    }
}

impl fmt::Display for ApiAbiFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "stage={} diagnostic={}",
            self.stage.as_str(),
            self.diagnostic
        )
    }
}

impl std::error::Error for ApiAbiFailure {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApiAbiValues {
    actual_api: u32,
    actual_engine: [u32; 4],
}

impl ApiAbiValues {
    pub(crate) const fn new(actual_api: u32, actual_engine: [u32; 4]) -> Self {
        Self {
            actual_api,
            actual_engine,
        }
    }
}

pub(crate) fn run_child(
    mode: ChildMode,
    runtime_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<(), ApiAbiFailure> {
    match mode {
        ChildMode::ApiAbi => run_api_abi_child_with(
            runtime_path,
            manifest,
            "arm64",
            crate::sciter::is_process_main_thread(),
            |emit| {
                let mut load_progress = |event| emit(load_event(event));
                let runtime = unsafe {
                    SciterRuntime::load_absolute_with_progress(
                        runtime_path,
                        runtime_path,
                        &mut load_progress,
                    )
                }
                .map_err(load_failure)?;

                let mut abi_progress = |event| emit(abi_event(event, manifest));
                let smoke = runtime
                    .abi_smoke_with_progress(manifest, &mut abi_progress)
                    .map_err(abi_failure)?;
                Ok(ApiAbiValues::new(
                    smoke.actual_api_version(),
                    smoke.actual_engine_version(),
                ))
            },
        ),
    }
}

pub(crate) fn run_api_abi_child_with<F>(
    _runtime_path: &Path,
    manifest: &ArtifactManifest,
    process_architecture: &str,
    is_main_thread: bool,
    probe: F,
) -> Result<(), ApiAbiFailure>
where
    F: FnOnce(&mut dyn FnMut(ProbeEvent)) -> Result<ApiAbiValues, ApiAbiFailure>,
{
    if process_architecture != "arm64" {
        return fail(
            manifest,
            process_architecture,
            is_main_thread,
            ApiAbiFailure::new(
                HarnessStage::ProcessArchitecture,
                format!(
                    "child process architecture {process_architecture} did not match expected arm64"
                ),
            ),
            Some(process_architecture),
            None,
            None,
        );
    }
    if !is_main_thread {
        return fail(
            manifest,
            process_architecture,
            is_main_thread,
            ApiAbiFailure::new(
                HarnessStage::ThreadContext,
                "child must run on the process main thread",
            ),
            None,
            None,
            None,
        );
    }

    let mut emit = |event| match event {
        ProbeEvent::Entered(stage) => progress(stage, "entered", "unavailable", "unavailable"),
        ProbeEvent::Completed {
            stage,
            actual,
            expected,
        } => progress(stage, "completed", &actual, &expected),
    };
    let values = match probe(&mut emit) {
        Ok(values) => values,
        Err(error) => {
            return fail(
                manifest,
                process_architecture,
                is_main_thread,
                error,
                None,
                None,
                None,
            )
        }
    };

    let actual_api = values.actual_api.to_string();
    if values.actual_api != manifest.api_version() {
        return fail(
            manifest,
            process_architecture,
            is_main_thread,
            ApiAbiFailure::new(
                HarnessStage::ApiVersion,
                format!(
                    "actual API version {} did not match expected {}",
                    values.actual_api,
                    manifest.api_version()
                ),
            ),
            Some(&actual_api),
            Some(&actual_api),
            Some(values.actual_engine),
        );
    }

    let actual_engine = version(values.actual_engine);
    let expected_engine = version(manifest.engine_version());
    if values.actual_engine != manifest.engine_version() {
        return fail(
            manifest,
            process_architecture,
            is_main_thread,
            ApiAbiFailure::new(
                HarnessStage::SciterVersionCall,
                format!(
                    "actual engine version {actual_engine} did not match expected {expected_engine}"
                ),
            ),
            Some(&actual_engine),
            Some(&actual_api),
            Some(values.actual_engine),
        );
    }

    result(
        "success_candidate",
        &actual_api,
        &manifest.api_version().to_string(),
        &actual_engine,
        &expected_engine,
        process_architecture,
        "main",
        "none",
    );
    Ok(())
}

fn progress(stage: HarnessStage, state: &str, actual: &str, expected: &str) {
    protocol_line(format!(
        "{PROTOCOL_PREFIX}\t{PROTOCOL_VERSION}\tapi_abi\tprogress\tstage={}\tstate={}\tactual={}\texpected={}",
        stage.as_str(),
        encode_value(state),
        encode_value(actual),
        encode_value(expected),
    ));
}

fn result(
    status: &str,
    actual_api: &str,
    expected_api: &str,
    actual_engine: &str,
    expected_engine: &str,
    process_architecture: &str,
    thread_context: &str,
    failure_stage: &str,
) {
    protocol_line(format!(
        "{PROTOCOL_PREFIX}\t{PROTOCOL_VERSION}\tapi_abi\tresult\tstatus={}\tactual_api={}\texpected_api={}\tactual_engine={}\texpected_engine={}\tscope={LIMITED_ABI_SCOPE}\tlifecycle_abi_validated=false\tprocess_architecture={}\tthread_context={}\tfailure_stage={}",
        encode_value(status),
        encode_value(actual_api),
        encode_value(expected_api),
        encode_value(actual_engine),
        encode_value(expected_engine),
        encode_value(process_architecture),
        encode_value(thread_context),
        encode_value(failure_stage),
    ));
}

fn protocol_line(line: String) {
    println!("{line}");
    io::stdout().flush().expect("flush child protocol progress");
}

fn fail(
    manifest: &ArtifactManifest,
    process_architecture: &str,
    is_main_thread: bool,
    error: ApiAbiFailure,
    progress_actual: Option<&str>,
    actual_api: Option<&str>,
    actual_engine: Option<[u32; 4]>,
) -> Result<(), ApiAbiFailure> {
    let (actual, expected) = failure_values(error.stage, progress_actual, manifest);
    progress(error.stage, "failed", &actual, &expected);
    result(
        "failure",
        actual_api.unwrap_or("unavailable"),
        &manifest.api_version().to_string(),
        &actual_engine
            .map(version)
            .unwrap_or_else(|| "unavailable".to_owned()),
        &version(manifest.engine_version()),
        process_architecture,
        if is_main_thread { "main" } else { "not_main" },
        error.stage.as_str(),
    );
    eprintln!(
        "api-abi child failed: stage={} diagnostic={}",
        error.stage.as_str(),
        one_line(&error.diagnostic)
    );
    Err(error)
}

fn failure_values(
    stage: HarnessStage,
    progress_actual: Option<&str>,
    manifest: &ArtifactManifest,
) -> (String, String) {
    match stage {
        HarnessStage::RuntimeLoad => ("failed".to_owned(), "loaded".to_owned()),
        HarnessStage::SciterApiExport => ("unresolved".to_owned(), "resolved".to_owned()),
        HarnessStage::ApiTable => ("null".to_owned(), "non_null".to_owned()),
        HarnessStage::ApiVersion => (
            progress_actual.unwrap_or("unavailable").to_owned(),
            manifest.api_version().to_string(),
        ),
        HarnessStage::SciterVersionCall => (
            progress_actual.unwrap_or("unavailable").to_owned(),
            version(manifest.engine_version()),
        ),
        HarnessStage::ProcessArchitecture => (
            progress_actual.unwrap_or("unavailable").to_owned(),
            "arm64".to_owned(),
        ),
        HarnessStage::ThreadContext => ("not_main".to_owned(), "main".to_owned()),
    }
}

fn load_event(event: RuntimeLoadProgress) -> ProbeEvent {
    match event {
        RuntimeLoadProgress::RuntimeLoadEntered => ProbeEvent::entered(HarnessStage::RuntimeLoad),
        RuntimeLoadProgress::RuntimeLoadCompleted => {
            ProbeEvent::completed(HarnessStage::RuntimeLoad, "loaded", "loaded")
        }
        RuntimeLoadProgress::SciterApiExportEntered => {
            ProbeEvent::entered(HarnessStage::SciterApiExport)
        }
        RuntimeLoadProgress::SciterApiExportCompleted => {
            ProbeEvent::completed(HarnessStage::SciterApiExport, "resolved", "resolved")
        }
        RuntimeLoadProgress::ApiTableEntered => ProbeEvent::entered(HarnessStage::ApiTable),
        RuntimeLoadProgress::ApiTableCompleted => {
            ProbeEvent::completed(HarnessStage::ApiTable, "non_null", "non_null")
        }
    }
}

fn abi_event(event: AbiSmokeProgress, manifest: &ArtifactManifest) -> ProbeEvent {
    match event {
        AbiSmokeProgress::ApiVersionEntered => ProbeEvent::entered(HarnessStage::ApiVersion),
        AbiSmokeProgress::ApiVersionCompleted(actual) => ProbeEvent::completed(
            HarnessStage::ApiVersion,
            actual.to_string(),
            manifest.api_version().to_string(),
        ),
        AbiSmokeProgress::SciterVersionCallEntered => {
            ProbeEvent::entered(HarnessStage::SciterVersionCall)
        }
        AbiSmokeProgress::SciterVersionCallCompleted(actual) => ProbeEvent::completed(
            HarnessStage::SciterVersionCall,
            version(actual),
            version(manifest.engine_version()),
        ),
    }
}

fn load_failure(error: RuntimeLoadError) -> ApiAbiFailure {
    let stage = match error {
        RuntimeLoadError::SymbolResolutionFailure { .. } => HarnessStage::SciterApiExport,
        RuntimeLoadError::NullApiTable => HarnessStage::ApiTable,
        _ => HarnessStage::RuntimeLoad,
    };
    ApiAbiFailure::new(stage, error.to_string())
}

fn abi_failure(error: RuntimeAbiError) -> ApiAbiFailure {
    let stage = match error {
        RuntimeAbiError::NotMainThread => HarnessStage::ThreadContext,
        RuntimeAbiError::NullSciterVersion => HarnessStage::SciterVersionCall,
    };
    ApiAbiFailure::new(stage, format!("{error:?}"))
}

fn version(value: [u32; 4]) -> String {
    format!("{}.{}.{}.{}", value[0], value[1], value[2], value[3])
}

fn encode_value(value: &str) -> String {
    let mut encoded = String::new();
    for character in value.chars() {
        match character {
            '%' => encoded.push_str("%25"),
            '=' => encoded.push_str("%3D"),
            '\t' => encoded.push_str("%09"),
            '\n' => encoded.push_str("%0A"),
            '\r' => encoded.push_str("%0D"),
            _ => encoded.push(character),
        }
    }
    encoded
}

fn one_line(diagnostic: &str) -> String {
    diagnostic
        .chars()
        .map(|character| {
            if matches!(character, '\n' | '\r' | '\t') {
                ' '
            } else {
                character
            }
        })
        .collect()
}
