use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::rc::Rc;

use crate::model::ArtifactManifest;
use crate::sciter::{
    AbiSmokeProgress, DebugCallbackContext, HostCallbackContext, LifecycleApi,
    RegisteredDebugContext, RegisteredHostContext, RuntimeAbiError, RuntimeLoadError,
    RuntimeLoadProgress, SciterRuntime, ShutdownComplete, WindowFlags, WindowHandle,
};

const PROTOCOL_PREFIX: &str = "MDLUMA_EVIDENCE";
const PROTOCOL_VERSION: u16 = 1;
pub(crate) const LIMITED_ABI_SCOPE: &str = "api_version+SciterVersion";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildMode {
    ApiAbi,
    PopupCycles,
}

#[derive(Debug)]
pub(crate) enum ChildFailure {
    ApiAbi(ApiAbiFailure),
    Popup(PopupFailure),
}

impl fmt::Display for ChildFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiAbi(error) => error.fmt(formatter),
            Self::Popup(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ChildFailure {}

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
    fixture_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<(), ChildFailure> {
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
        )
        .map_err(ChildFailure::ApiAbi),
        ChildMode::PopupCycles => {
            run_popup_child(runtime_path, fixture_path).map_err(ChildFailure::Popup)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PopupFailure {
    Fixture(String),
    Lifecycle(String),
    Protocol(String),
    PrimaryAndShutdown {
        primary: Box<PopupFailure>,
        shutdown: Box<PopupFailure>,
    },
}

impl fmt::Display for PopupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixture(diagnostic) => write!(formatter, "popup fixture: {diagnostic}"),
            Self::Lifecycle(diagnostic) => write!(formatter, "popup lifecycle: {diagnostic}"),
            Self::Protocol(diagnostic) => write!(formatter, "popup protocol: {diagnostic}"),
            Self::PrimaryAndShutdown { primary, shutdown } => {
                write!(
                    formatter,
                    "{primary}; shutdown cleanup also failed: {shutdown}"
                )
            }
        }
    }
}

impl std::error::Error for PopupFailure {}

#[derive(Default)]
pub(crate) struct PopupProtocolValidator {
    next_cycle: u16,
    next_phase: u8,
    failure: Option<String>,
}

impl PopupProtocolValidator {
    pub(crate) fn accept_utf16(&mut self, text: &[u16]) -> Result<String, String> {
        let line = String::from_utf16(text)
            .map_err(|error| self.reject(format!("debug text is not valid UTF-16: {error}")))?;
        self.accept_line(&line)?;
        Ok(line)
    }

    fn accept_line(&mut self, text: &str) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let line = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(text);
        if line.contains(['\r', '\n']) {
            return Err(self.reject("protocol callback contained multiple lines"));
        }
        let columns: Vec<_> = line.split('\t').collect();
        if columns.len() != 5
            || columns[0] != PROTOCOL_PREFIX
            || columns[1] != PROTOCOL_VERSION.to_string()
            || columns[2] != "popup"
        {
            return Err(self.reject("invalid popup protocol envelope"));
        }
        let cycle = columns[3]
            .parse::<u16>()
            .map_err(|_| self.reject("popup cycle is not a decimal u16"))?;
        if columns[3] != cycle.to_string() || !(1..=100).contains(&cycle) {
            return Err(self.reject("popup cycle must be canonical decimal 1..100"));
        }
        let expected_cycle = if self.next_cycle == 0 {
            1
        } else {
            self.next_cycle
        };
        let expected_phase = ["started", "shown", "closed"][self.next_phase as usize];
        if cycle != expected_cycle || columns[4] != expected_phase {
            return Err(self.reject(format!(
                "expected popup cycle {expected_cycle} phase {expected_phase}, got cycle {cycle} phase {}",
                columns[4]
            )));
        }
        if self.next_phase == 2 {
            self.next_phase = 0;
            self.next_cycle = cycle + 1;
        } else {
            self.next_phase += 1;
            self.next_cycle = cycle;
        }
        Ok(())
    }

    fn reject(&mut self, diagnostic: impl Into<String>) -> String {
        let diagnostic = diagnostic.into();
        if self.failure.is_none() {
            self.failure = Some(diagnostic.clone());
        }
        diagnostic
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn complete(&self) -> bool {
        self.failure.is_none() && self.next_cycle == 101 && self.next_phase == 0
    }
}

pub(crate) trait PopupLifecycleApi {
    type Error: fmt::Display;

    fn init(&mut self) -> Result<(), Self::Error>;
    fn create_controller(&mut self) -> Result<(), Self::Error>;
    fn register_controller_callback(&mut self) -> Result<(), Self::Error>;
    fn register_debug_output(&mut self) -> Result<(), Self::Error>;
    fn load_fixture(&mut self, html: &[u8], base_url: &[u16]) -> Result<(), Self::Error>;
    fn loop_iteration(&mut self) -> Result<bool, Self::Error>;
    fn controller_destroyed(&self) -> Result<bool, Self::Error>;
    fn protocol_failure(&self) -> Option<String>;
    fn protocol_complete(&self) -> bool;
    fn debug_failure(&self) -> Result<Option<String>, Self::Error>;
    fn release_controller_context(&mut self);
    fn stop(&mut self) -> Result<(), Self::Error>;
    fn shutdown(&mut self) -> Result<(), Self::Error>;
    fn release_debug_context(&mut self);
}

pub(crate) fn run_popup_cycles_with<A: PopupLifecycleApi>(
    api: &mut A,
    fixture: &[u8],
    base_url: &[u16],
) -> Result<(), PopupFailure> {
    api.init().map_err(lifecycle_failure)?;
    let result = (|| {
        api.create_controller().map_err(lifecycle_failure)?;
        api.register_controller_callback()
            .map_err(lifecycle_failure)?;
        api.register_debug_output().map_err(lifecycle_failure)?;
        api.load_fixture(fixture, base_url)
            .map_err(lifecycle_failure)?;

        let continued_after_destroy = loop {
            let continued = api.loop_iteration().map_err(lifecycle_failure)?;
            if api.controller_destroyed().map_err(lifecycle_failure)? {
                break continued;
            }
            if !continued {
                return Err(PopupFailure::Lifecycle(
                    "event loop stopped before controller destruction".to_owned(),
                ));
            }
        };
        api.release_controller_context();

        if let Some(failure) = api.debug_failure().map_err(lifecycle_failure)? {
            return Err(PopupFailure::Protocol(failure));
        }
        if let Some(failure) = api.protocol_failure() {
            return Err(PopupFailure::Protocol(failure));
        }
        if !api.protocol_complete() {
            return Err(PopupFailure::Protocol(
                "controller was destroyed before exactly 100 popup cycles completed".to_owned(),
            ));
        }
        if continued_after_destroy {
            api.stop().map_err(lifecycle_failure)?;
            if api.loop_iteration().map_err(lifecycle_failure)? {
                return Err(PopupFailure::Lifecycle(
                    "event loop continued after the single STOP command".to_owned(),
                ));
            }
        }
        Ok(())
    })();

    let shutdown = api.shutdown().map_err(lifecycle_failure);
    if shutdown.is_ok() {
        api.release_debug_context();
    }
    match (result, shutdown) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(shutdown)) => Err(shutdown),
        (Err(primary), Err(shutdown)) => Err(PopupFailure::PrimaryAndShutdown {
            primary: Box::new(primary),
            shutdown: Box::new(shutdown),
        }),
    }
}

fn lifecycle_failure(error: impl fmt::Display) -> PopupFailure {
    PopupFailure::Lifecycle(error.to_string())
}

struct SciterPopupApi<'a> {
    api: LifecycleApi,
    controller: Option<WindowHandle>,
    host: Option<RegisteredHostContext>,
    debug: Option<RegisteredDebugContext>,
    protocol: Rc<RefCell<PopupProtocolValidator>>,
    shutdown: Option<ShutdownComplete>,
    _runtime: &'a SciterRuntime,
}

impl<'a> SciterPopupApi<'a> {
    fn new(runtime: &'a SciterRuntime) -> Result<Self, PopupFailure> {
        Ok(Self {
            api: runtime.lifecycle_api().map_err(lifecycle_failure)?,
            controller: None,
            host: None,
            debug: None,
            protocol: Rc::new(RefCell::new(PopupProtocolValidator::default())),
            shutdown: None,
            _runtime: runtime,
        })
    }
}

impl PopupLifecycleApi for SciterPopupApi<'_> {
    type Error = PopupFailure;

    fn init(&mut self) -> Result<(), Self::Error> {
        self.api.init().map_err(lifecycle_failure)?;
        Ok(())
    }

    fn create_controller(&mut self) -> Result<(), Self::Error> {
        self.controller = Some(
            self.api
                .create_window(WindowFlags::MAIN, None, None)
                .map_err(lifecycle_failure)?,
        );
        Ok(())
    }

    fn register_controller_callback(&mut self) -> Result<(), Self::Error> {
        self.host = Some(
            self.api
                .register_host_callback(self.controller.unwrap(), HostCallbackContext::new())
                .map_err(lifecycle_failure)?,
        );
        Ok(())
    }

    fn register_debug_output(&mut self) -> Result<(), Self::Error> {
        let protocol = Rc::clone(&self.protocol);
        let debug = DebugCallbackContext::with_handler(PROTOCOL_PREFIX, move |text| {
            let line = text
                .strip_suffix("\r\n")
                .or_else(|| text.strip_suffix('\n'))
                .unwrap_or(text);
            if line.starts_with(PROTOCOL_PREFIX) {
                protocol.borrow_mut().accept_line(text)?;
                let mut stdout = io::stdout().lock();
                writeln!(stdout, "{line}").map_err(|error| error.to_string())?;
                stdout.flush().map_err(|error| error.to_string())
            } else {
                let mut stderr = io::stderr().lock();
                let _ = writeln!(stderr, "popup child diagnostic: {}", one_line(text));
                Ok(())
            }
        });
        self.debug = Some(
            self.api
                .register_debug_output(Some(self.controller.unwrap()), debug)
                .map_err(lifecycle_failure)?,
        );
        Ok(())
    }

    fn load_fixture(&mut self, html: &[u8], base_url: &[u16]) -> Result<(), Self::Error> {
        self.api
            .load_html(self.controller.unwrap(), html, Some(base_url))
            .map_err(lifecycle_failure)?;
        Ok(())
    }

    fn loop_iteration(&mut self) -> Result<bool, Self::Error> {
        self.api.loop_iteration().map_err(lifecycle_failure)
    }

    fn controller_destroyed(&self) -> Result<bool, Self::Error> {
        self.host
            .as_ref()
            .ok_or_else(|| {
                PopupFailure::Lifecycle("controller callback is not registered".to_owned())
            })?
            .destroyed()
            .map_err(lifecycle_failure)
    }

    fn protocol_failure(&self) -> Option<String> {
        self.protocol.borrow().failure().map(str::to_owned)
    }

    fn protocol_complete(&self) -> bool {
        self.protocol.borrow().complete()
    }

    fn debug_failure(&self) -> Result<Option<String>, Self::Error> {
        self.debug
            .as_ref()
            .map(|debug| {
                debug
                    .failure()
                    .map(|failure| failure.map(str::to_owned))
                    .map_err(lifecycle_failure)
            })
            .unwrap_or(Ok(None))
    }

    fn release_controller_context(&mut self) {
        drop(self.host.take());
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.api.stop().map_err(lifecycle_failure)?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.shutdown = Some(self.api.shutdown().map_err(lifecycle_failure)?);
        Ok(())
    }

    fn release_debug_context(&mut self) {
        if let (Some(debug), Some(shutdown)) = (self.debug.take(), self.shutdown.take()) {
            debug.release_after_shutdown(shutdown);
        }
    }
}

pub(crate) fn run_popup_child(
    runtime_path: &Path,
    fixture_path: &Path,
) -> Result<(), PopupFailure> {
    let fixture_path = fixture_path
        .canonicalize()
        .map_err(|error| PopupFailure::Fixture(error.to_string()))?;
    let fixture =
        fs::read(&fixture_path).map_err(|error| PopupFailure::Fixture(error.to_string()))?;
    let base_url = file_url_utf16(&fixture_path)?;
    let runtime = unsafe { SciterRuntime::load_absolute(runtime_path, runtime_path) }
        .map_err(|error| PopupFailure::Lifecycle(error.to_string()))?;
    let mut api = SciterPopupApi::new(&runtime)?;
    run_popup_cycles_with(&mut api, &fixture, &base_url)
}

fn file_url_utf16(path: &Path) -> Result<Vec<u16>, PopupFailure> {
    if !path.is_absolute() {
        return Err(PopupFailure::Fixture(
            "popup fixture path must be absolute".to_owned(),
        ));
    }
    let mut url = String::from("file://");
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            url.push(*byte as char);
        } else {
            use std::fmt::Write as _;
            write!(url, "%{byte:02X}").unwrap();
        }
    }
    let mut encoded: Vec<_> = url.encode_utf16().collect();
    encoded.push(0);
    Ok(encoded)
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

#[cfg(test)]
mod popup_tests {
    use super::{run_popup_cycles_with, PopupFailure, PopupLifecycleApi, PopupProtocolValidator};

    #[derive(Default)]
    struct SyntheticPopupApi {
        calls: Vec<&'static str>,
        messages: Vec<Vec<u16>>,
        iteration_results: Vec<bool>,
        destroyed_after_iteration: usize,
        iterations: usize,
        destroyed: bool,
        protocol: PopupProtocolValidator,
        reject_stop: bool,
        shutdown_main_thread_failure: bool,
        reject_debug_query: bool,
    }

    impl SyntheticPopupApi {
        fn success(loop_continues_after_destroy: bool) -> Self {
            let mut messages = Vec::new();
            for cycle in 1..=100 {
                for phase in ["started", "shown", "closed"] {
                    messages.push(
                        format!("MDLUMA_EVIDENCE\t1\tpopup\t{cycle}\t{phase}")
                            .encode_utf16()
                            .collect(),
                    );
                }
            }
            Self {
                messages,
                iteration_results: if loop_continues_after_destroy {
                    vec![true, false]
                } else {
                    vec![false]
                },
                destroyed_after_iteration: 1,
                ..Self::default()
            }
        }
    }

    impl PopupLifecycleApi for SyntheticPopupApi {
        type Error = &'static str;

        fn init(&mut self) -> Result<(), Self::Error> {
            self.calls.push("init");
            Ok(())
        }

        fn create_controller(&mut self) -> Result<(), Self::Error> {
            self.calls.push("create");
            Ok(())
        }

        fn register_controller_callback(&mut self) -> Result<(), Self::Error> {
            self.calls.push("callback");
            Ok(())
        }

        fn register_debug_output(&mut self) -> Result<(), Self::Error> {
            self.calls.push("debug");
            Ok(())
        }

        fn load_fixture(&mut self, html: &[u8], base_url: &[u16]) -> Result<(), Self::Error> {
            self.calls.push("load");
            assert_eq!(html, b"exact popup fixture");
            assert_eq!(base_url.last(), Some(&0));
            Ok(())
        }

        fn loop_iteration(&mut self) -> Result<bool, Self::Error> {
            self.calls.push("iteration");
            self.iterations += 1;
            for message in std::mem::take(&mut self.messages) {
                let _ = self.protocol.accept_utf16(&message);
            }
            if self.iterations >= self.destroyed_after_iteration {
                self.destroyed = true;
            }
            Ok(self.iteration_results.remove(0))
        }

        fn controller_destroyed(&self) -> Result<bool, Self::Error> {
            Ok(self.destroyed)
        }

        fn protocol_failure(&self) -> Option<String> {
            self.protocol.failure().map(str::to_owned)
        }

        fn protocol_complete(&self) -> bool {
            self.protocol.complete()
        }

        fn debug_failure(&self) -> Result<Option<String>, Self::Error> {
            if self.reject_debug_query {
                Err("debug context query failed")
            } else {
                Ok(None)
            }
        }

        fn release_controller_context(&mut self) {
            self.calls.push("release_controller");
        }

        fn stop(&mut self) -> Result<(), Self::Error> {
            self.calls.push("stop");
            if self.reject_stop {
                Err("STOP rejected")
            } else {
                Ok(())
            }
        }

        fn shutdown(&mut self) -> Result<(), Self::Error> {
            self.calls.push("shutdown");
            if self.shutdown_main_thread_failure {
                Err("SHUTDOWN failed main-thread check")
            } else {
                Ok(())
            }
        }

        fn release_debug_context(&mut self) {
            self.calls.push("release_debug");
        }
    }

    #[test]
    fn popup_cycles_keep_fixed_order_and_stop_once_only_when_loop_continues() {
        for (continues, expected_tail) in [
            (
                true,
                vec![
                    "release_controller",
                    "stop",
                    "iteration",
                    "shutdown",
                    "release_debug",
                ],
            ),
            (
                false,
                vec!["release_controller", "shutdown", "release_debug"],
            ),
        ] {
            let mut api = SyntheticPopupApi::success(continues);
            run_popup_cycles_with(
                &mut api,
                b"exact popup fixture",
                &"file:///fixed/popup.htm\0"
                    .encode_utf16()
                    .collect::<Vec<_>>(),
            )
            .unwrap();

            assert_eq!(
                &api.calls[..5],
                ["init", "create", "callback", "debug", "load"]
            );
            assert_eq!(
                &api.calls[api.calls.len() - expected_tail.len()..],
                expected_tail
            );
            assert_eq!(
                api.calls.iter().filter(|call| **call == "stop").count(),
                continues as usize
            );
        }
    }

    #[test]
    fn popup_protocol_accepts_exactly_one_ordered_triplet_for_cycles_one_through_100() {
        let mut protocol = PopupProtocolValidator::default();
        for cycle in 1..=100 {
            for phase in ["started", "shown", "closed"] {
                protocol
                    .accept_utf16(
                        &format!("MDLUMA_EVIDENCE\t1\tpopup\t{cycle}\t{phase}")
                            .encode_utf16()
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();
            }
        }
        assert!(protocol.complete());

        for malformed in [
            "MDLUMA_EVIDENCE\t1\tpopup\t1\tclosed",
            "MDLUMA_EVIDENCE\t1\tpopup\t0\tstarted",
            "MDLUMA_EVIDENCE\t1\twindow\t1\tstarted",
            "MDLUMA_EVIDENCE\t2\tpopup\t1\tstarted",
            "MDLUMA_EVIDENCE\t1\tpopup\t01\tstarted",
            "MDLUMA_EVIDENCE\t1\tpopup\t1\tstarted\textra",
        ] {
            assert!(PopupProtocolValidator::default()
                .accept_utf16(&malformed.encode_utf16().collect::<Vec<_>>())
                .is_err());
        }
        assert!(PopupProtocolValidator::default()
            .accept_utf16(&[0xd800])
            .is_err());
    }

    #[test]
    fn malformed_popup_sequence_fails_but_releases_contexts_and_shuts_down() {
        let mut api = SyntheticPopupApi::success(false);
        api.messages = [
            "MDLUMA_EVIDENCE\t1\tpopup\t1\tstarted",
            "MDLUMA_EVIDENCE\t1\tpopup\t1\tclosed",
        ]
        .into_iter()
        .map(|line| line.encode_utf16().collect())
        .collect();

        assert!(matches!(
            run_popup_cycles_with(
                &mut api,
                b"exact popup fixture",
                &"file:///fixed/popup.htm\0"
                    .encode_utf16()
                    .collect::<Vec<_>>(),
            ),
            Err(PopupFailure::Protocol(_))
        ));
        assert_eq!(
            &api.calls[api.calls.len() - 3..],
            ["release_controller", "shutdown", "release_debug"]
        );
    }

    #[test]
    fn popup_cycles_fail_when_loop_continues_after_the_single_stop() {
        let mut api = SyntheticPopupApi::success(true);
        api.iteration_results = vec![true, true];

        assert!(matches!(
            run_popup_cycles_with(
                &mut api,
                b"exact popup fixture",
                &"file:///fixed/popup.htm\0".encode_utf16().collect::<Vec<_>>(),
            ),
            Err(PopupFailure::Lifecycle(diagnostic))
                if diagnostic.contains("continued after the single STOP")
        ));
        assert_eq!(api.calls.iter().filter(|call| **call == "stop").count(), 1);
        assert_eq!(
            &api.calls[api.calls.len() - 2..],
            ["shutdown", "release_debug"]
        );
    }

    #[test]
    fn rejected_stop_result_does_not_claim_completion() {
        let mut rejected_stop = SyntheticPopupApi::success(true);
        rejected_stop.reject_stop = true;
        assert!(matches!(
            run_popup_cycles_with(
                &mut rejected_stop,
                b"exact popup fixture",
                &"file:///fixed/popup.htm\0".encode_utf16().collect::<Vec<_>>(),
            ),
            Err(PopupFailure::Lifecycle(diagnostic)) if diagnostic.contains("STOP rejected")
        ));
        assert_eq!(
            &rejected_stop.calls[rejected_stop.calls.len() - 3..],
            ["stop", "shutdown", "release_debug"]
        );
    }

    #[test]
    fn debug_context_query_failure_is_propagated_and_cleanup_remains_safe() {
        let mut api = SyntheticPopupApi::success(false);
        api.reject_debug_query = true;

        assert!(matches!(
            run_popup_cycles_with(
                &mut api,
                b"exact popup fixture",
                &"file:///fixed/popup.htm\0".encode_utf16().collect::<Vec<_>>(),
            ),
            Err(PopupFailure::Lifecycle(diagnostic))
                if diagnostic.contains("debug context query failed")
        ));
        assert_eq!(
            &api.calls[api.calls.len() - 3..],
            ["release_controller", "shutdown", "release_debug"]
        );
    }

    #[test]
    fn primary_and_shutdown_failures_are_both_preserved() {
        let mut api = SyntheticPopupApi::success(false);
        api.messages = [
            "MDLUMA_EVIDENCE\t1\tpopup\t1\tstarted",
            "MDLUMA_EVIDENCE\t1\tpopup\t1\tclosed",
        ]
        .into_iter()
        .map(|line| line.encode_utf16().collect())
        .collect();
        api.shutdown_main_thread_failure = true;

        let error = run_popup_cycles_with(
            &mut api,
            b"exact popup fixture",
            &"file:///fixed/popup.htm\0"
                .encode_utf16()
                .collect::<Vec<_>>(),
        )
        .unwrap_err();

        assert!(matches!(
            &error,
            PopupFailure::PrimaryAndShutdown { primary, shutdown }
                if matches!(primary.as_ref(), PopupFailure::Protocol(_))
                    && matches!(shutdown.as_ref(), PopupFailure::Lifecycle(diagnostic)
                        if diagnostic.contains("main-thread"))
        ));
        let diagnostic = error.to_string();

        assert!(diagnostic.contains("expected popup cycle 1 phase shown"));
        assert!(diagnostic.contains("SHUTDOWN failed main-thread check"));
    }
}
