use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArtifactManifest {
    pub(crate) schema_version: u32,
    pub(crate) repository: String,
    pub(crate) commit: String,
    pub(crate) sdk_relative_path: PathBuf,
    pub(crate) workspace_relative_path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) engine_version: [u32; 4],
    pub(crate) api_version: u32,
    pub(crate) version_header_path: PathBuf,
    pub(crate) api_header_path: PathBuf,
    pub(crate) version_header_source: String,
    pub(crate) api_header_source: String,
}

impl ArtifactManifest {
    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) fn repository(&self) -> &str {
        &self.repository
    }

    pub(crate) fn commit(&self) -> &str {
        &self.commit
    }

    pub(crate) fn sdk_relative_path(&self) -> &std::path::Path {
        &self.sdk_relative_path
    }

    pub(crate) fn workspace_relative_path(&self) -> &std::path::Path {
        &self.workspace_relative_path
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    pub(crate) const fn engine_version(&self) -> [u32; 4] {
        self.engine_version
    }

    pub(crate) const fn api_version(&self) -> u32 {
        self.api_version
    }

    pub(crate) fn version_header_path(&self) -> &std::path::Path {
        &self.version_header_path
    }

    pub(crate) fn api_header_path(&self) -> &std::path::Path {
        &self.api_header_path
    }

    pub(crate) fn version_header_source(&self) -> &str {
        &self.version_header_source
    }

    pub(crate) fn api_header_source(&self) -> &str {
        &self.api_header_source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CriterionStatus {
    Satisfied,
    Unsatisfied,
    NotRun,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CriterionId {
    requirement: u8,
    criterion: u8,
}

impl CriterionId {
    pub(crate) const fn from_parts(requirement: u8, criterion: u8) -> Self {
        Self {
            requirement,
            criterion,
        }
    }

    pub(crate) const fn requirement(self) -> u8 {
        self.requirement
    }

    pub(crate) const fn criterion(self) -> u8 {
        self.criterion
    }

    pub(crate) fn is_known(self) -> bool {
        ALL_CRITERIA.contains(&self)
    }

    pub(crate) fn permits_not_applicable(self) -> bool {
        CONDITIONAL_CRITERIA.contains(&self)
    }
}

impl fmt::Display for CriterionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.requirement, self.criterion)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CriterionResult {
    id: CriterionId,
    status: CriterionStatus,
    summary: String,
    artifacts: Vec<PathBuf>,
}

impl CriterionResult {
    pub(crate) fn new(
        id: CriterionId,
        status: CriterionStatus,
        summary: impl Into<String>,
        artifacts: Vec<PathBuf>,
    ) -> Self {
        Self {
            id,
            status,
            summary: summary.into(),
            artifacts,
        }
    }

    pub(crate) fn try_new(
        id: CriterionId,
        status: CriterionStatus,
        summary: impl Into<String>,
        artifacts: Vec<PathBuf>,
    ) -> Result<Self, EvidenceError> {
        if !id.is_known() {
            return Err(EvidenceError::UnknownCriterion(id));
        }
        if status == CriterionStatus::NotApplicable && !id.permits_not_applicable() {
            return Err(EvidenceError::NotApplicableNotAllowed(id));
        }
        Ok(Self::new(id, status, summary, artifacts))
    }

    pub(crate) const fn id(&self) -> CriterionId {
        self.id
    }

    pub(crate) const fn status(&self) -> CriterionStatus {
        self.status
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    pub(crate) fn artifacts(&self) -> &[PathBuf] {
        &self.artifacts
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum GateId {
    Artifact,
    Platform,
    Api,
    Abi,
    Popup,
    Window,
    License,
    Record,
}

impl GateId {
    pub(crate) const fn criteria(self) -> &'static [CriterionId] {
        match self {
            Self::Artifact => &ARTIFACT_CRITERIA,
            Self::Platform => &PLATFORM_CRITERIA,
            Self::Api => &API_CRITERIA,
            Self::Abi => &ABI_CRITERIA,
            Self::Popup => &POPUP_CRITERIA,
            Self::Window => &WINDOW_CRITERIA,
            Self::License => &LICENSE_CRITERIA,
            Self::Record => &RECORD_CRITERIA,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GateStatus {
    Pass,
    Fail,
    NotRun,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GateResult {
    id: GateId,
    status: GateStatus,
    criteria: Vec<CriterionId>,
    summary: String,
}

impl GateResult {
    pub(crate) fn new(
        id: GateId,
        status: GateStatus,
        criteria: Vec<CriterionId>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            id,
            status,
            criteria,
            summary: summary.into(),
        }
    }

    pub(crate) const fn id(&self) -> GateId {
        self.id
    }

    pub(crate) const fn status(&self) -> GateStatus {
        self.status
    }

    pub(crate) fn criteria(&self) -> &[CriterionId] {
        &self.criteria
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DecisionState {
    Pending,
    Go,
    NoGo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CycleKind {
    Popup,
    Window,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CyclePhase {
    Started,
    Shown,
    Closed,
    Created,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HarnessEvent {
    protocol_version: u16,
    kind: CycleKind,
    cycle: u16,
    phase: CyclePhase,
}

impl HarnessEvent {
    pub(crate) fn new(
        protocol_version: u16,
        kind: CycleKind,
        cycle: u16,
        phase: CyclePhase,
    ) -> Result<Self, EvidenceError> {
        if protocol_version != 1 {
            return Err(EvidenceError::Protocol(format!(
                "unsupported child protocol version: {protocol_version}"
            )));
        }
        if !(1..=100).contains(&cycle) {
            return Err(EvidenceError::InvalidCycle(cycle));
        }
        let legal_phase = match kind {
            CycleKind::Popup => matches!(
                phase,
                CyclePhase::Started | CyclePhase::Shown | CyclePhase::Closed
            ),
            CycleKind::Window => matches!(
                phase,
                CyclePhase::Started | CyclePhase::Created | CyclePhase::Destroyed
            ),
        };
        if !legal_phase {
            return Err(EvidenceError::InvalidCyclePhase { kind, phase });
        }
        Ok(Self {
            protocol_version,
            kind,
            cycle,
            phase,
        })
    }

    pub(crate) const fn protocol_version(self) -> u16 {
        self.protocol_version
    }

    pub(crate) const fn kind(self) -> CycleKind {
        self.kind
    }

    pub(crate) const fn cycle(self) -> u16 {
        self.cycle
    }

    pub(crate) const fn phase(self) -> CyclePhase {
        self.phase
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RunId(String);

impl RunId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, EvidenceError> {
        let value = value.into();
        if value.is_empty()
            || value == "."
            || value == ".."
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(EvidenceError::InvalidRunId(value));
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) enum EvidenceError {
    InvalidManifest(String),
    UnsupportedHost(String),
    CommandFailure {
        command: String,
        diagnostic: String,
    },
    RuntimeLoad(String),
    Protocol(String),
    Timeout {
        kind: CycleKind,
        cycle: u16,
    },
    ChildExit {
        code: Option<i32>,
        signal: Option<i32>,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    UnknownCriterion(CriterionId),
    DuplicateCriterion(CriterionId),
    MissingCriterion(CriterionId),
    NotApplicableNotAllowed(CriterionId),
    InvalidCycle(u16),
    InvalidCyclePhase {
        kind: CycleKind,
        phase: CyclePhase,
    },
    InvalidRunId(String),
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for EvidenceError {}

pub(crate) fn validate_criterion_results(
    results: &[CriterionResult],
    expected: &[CriterionId],
) -> Result<(), EvidenceError> {
    for result in results {
        if !result.id.is_known() || !expected.contains(&result.id) {
            return Err(EvidenceError::UnknownCriterion(result.id));
        }
        if result.status == CriterionStatus::NotApplicable && !result.id.permits_not_applicable() {
            return Err(EvidenceError::NotApplicableNotAllowed(result.id));
        }
    }

    for (index, result) in results.iter().enumerate() {
        if results[..index]
            .iter()
            .any(|candidate| candidate.id == result.id)
        {
            return Err(EvidenceError::DuplicateCriterion(result.id));
        }
    }

    for id in expected {
        if !results.iter().any(|result| result.id == *id) {
            return Err(EvidenceError::MissingCriterion(*id));
        }
    }
    Ok(())
}

const fn criterion(requirement: u8, criterion: u8) -> CriterionId {
    CriterionId::from_parts(requirement, criterion)
}

pub(crate) const ALL_CRITERIA: &[CriterionId] = &[
    criterion(1, 1),
    criterion(1, 2),
    criterion(1, 3),
    criterion(1, 4),
    criterion(1, 5),
    criterion(1, 6),
    criterion(1, 7),
    criterion(2, 1),
    criterion(2, 2),
    criterion(2, 3),
    criterion(2, 4),
    criterion(2, 5),
    criterion(2, 6),
    criterion(2, 7),
    criterion(2, 8),
    criterion(3, 1),
    criterion(3, 2),
    criterion(3, 3),
    criterion(3, 4),
    criterion(3, 5),
    criterion(3, 6),
    criterion(3, 7),
    criterion(3, 8),
    criterion(3, 9),
    criterion(4, 1),
    criterion(4, 2),
    criterion(4, 3),
    criterion(4, 4),
    criterion(4, 5),
    criterion(4, 6),
    criterion(4, 7),
    criterion(4, 8),
    criterion(4, 9),
    criterion(4, 10),
    criterion(5, 1),
    criterion(5, 2),
    criterion(5, 3),
    criterion(5, 4),
    criterion(5, 5),
    criterion(5, 6),
    criterion(5, 7),
    criterion(5, 8),
    criterion(5, 9),
    criterion(5, 10),
    criterion(6, 1),
    criterion(6, 2),
    criterion(6, 3),
    criterion(6, 4),
    criterion(6, 5),
    criterion(6, 6),
    criterion(6, 7),
    criterion(6, 8),
    criterion(6, 9),
    criterion(6, 10),
    criterion(6, 11),
    criterion(7, 1),
    criterion(7, 2),
    criterion(7, 3),
    criterion(7, 4),
    criterion(7, 5),
    criterion(7, 6),
    criterion(7, 7),
    criterion(7, 8),
    criterion(7, 9),
    criterion(7, 10),
    criterion(7, 11),
    criterion(7, 12),
    criterion(8, 1),
    criterion(8, 2),
    criterion(8, 3),
    criterion(8, 4),
    criterion(8, 5),
    criterion(8, 6),
    criterion(8, 7),
    criterion(8, 8),
    criterion(8, 9),
    criterion(8, 10),
    criterion(8, 11),
];

pub(crate) const SOURCE_CRITERIA: &[CriterionId] = &[
    criterion(1, 1),
    criterion(1, 2),
    criterion(1, 3),
    criterion(1, 4),
    criterion(1, 5),
    criterion(1, 6),
    criterion(1, 7),
    criterion(2, 1),
    criterion(2, 2),
    criterion(2, 3),
    criterion(2, 4),
    criterion(2, 5),
    criterion(2, 6),
    criterion(2, 7),
    criterion(2, 8),
    criterion(3, 1),
    criterion(3, 2),
    criterion(3, 3),
    criterion(3, 4),
    criterion(3, 5),
    criterion(3, 6),
    criterion(3, 7),
    criterion(3, 8),
    criterion(3, 9),
    criterion(4, 1),
    criterion(4, 2),
    criterion(4, 3),
    criterion(4, 4),
    criterion(4, 5),
    criterion(4, 6),
    criterion(4, 7),
    criterion(4, 8),
    criterion(4, 9),
    criterion(4, 10),
    criterion(5, 1),
    criterion(5, 2),
    criterion(5, 3),
    criterion(5, 4),
    criterion(5, 5),
    criterion(5, 6),
    criterion(5, 7),
    criterion(5, 8),
    criterion(5, 9),
    criterion(5, 10),
    criterion(6, 1),
    criterion(6, 2),
    criterion(6, 3),
    criterion(6, 4),
    criterion(6, 5),
    criterion(6, 6),
    criterion(6, 7),
    criterion(6, 8),
    criterion(6, 9),
    criterion(6, 10),
    criterion(6, 11),
    criterion(7, 1),
    criterion(7, 2),
    criterion(7, 3),
    criterion(7, 4),
    criterion(7, 5),
    criterion(7, 6),
    criterion(7, 7),
    criterion(7, 8),
    criterion(7, 9),
    criterion(7, 10),
    criterion(7, 11),
    criterion(7, 12),
];

// These are exactly the acceptance criteria introduced by a When or If trigger.
const CONDITIONAL_CRITERIA: &[CriterionId] = &[
    criterion(1, 5),
    criterion(1, 6),
    criterion(1, 7),
    criterion(2, 1),
    criterion(2, 2),
    criterion(2, 3),
    criterion(2, 4),
    criterion(3, 3),
    criterion(3, 4),
    criterion(3, 5),
    criterion(3, 6),
    criterion(3, 7),
    criterion(3, 8),
    criterion(3, 9),
    criterion(4, 3),
    criterion(4, 4),
    criterion(4, 5),
    criterion(4, 6),
    criterion(4, 7),
    criterion(4, 8),
    criterion(4, 9),
    criterion(5, 2),
    criterion(5, 3),
    criterion(5, 6),
    criterion(5, 7),
    criterion(5, 8),
    criterion(5, 9),
    criterion(6, 7),
    criterion(6, 8),
    criterion(6, 9),
    criterion(6, 10),
    criterion(6, 11),
    criterion(7, 10),
    criterion(7, 11),
    criterion(7, 12),
    criterion(8, 2),
    criterion(8, 3),
    criterion(8, 4),
    criterion(8, 5),
    criterion(8, 6),
    criterion(8, 7),
    criterion(8, 8),
    criterion(8, 9),
    criterion(8, 10),
];

const ARTIFACT_CRITERIA: [CriterionId; 7] = [
    criterion(1, 1),
    criterion(1, 2),
    criterion(1, 3),
    criterion(1, 4),
    criterion(1, 5),
    criterion(1, 6),
    criterion(1, 7),
];
const PLATFORM_CRITERIA: [CriterionId; 8] = [
    criterion(2, 1),
    criterion(2, 2),
    criterion(2, 3),
    criterion(2, 4),
    criterion(2, 5),
    criterion(2, 6),
    criterion(2, 7),
    criterion(2, 8),
];
const API_CRITERIA: [CriterionId; 9] = [
    criterion(3, 1),
    criterion(3, 2),
    criterion(3, 3),
    criterion(3, 4),
    criterion(3, 5),
    criterion(3, 6),
    criterion(3, 7),
    criterion(3, 8),
    criterion(3, 9),
];
const ABI_CRITERIA: [CriterionId; 10] = [
    criterion(4, 1),
    criterion(4, 2),
    criterion(4, 3),
    criterion(4, 4),
    criterion(4, 5),
    criterion(4, 6),
    criterion(4, 7),
    criterion(4, 8),
    criterion(4, 9),
    criterion(4, 10),
];
const POPUP_CRITERIA: [CriterionId; 7] = [
    criterion(5, 1),
    criterion(5, 2),
    criterion(5, 4),
    criterion(5, 6),
    criterion(5, 8),
    criterion(5, 9),
    criterion(5, 10),
];
const WINDOW_CRITERIA: [CriterionId; 7] = [
    criterion(5, 1),
    criterion(5, 3),
    criterion(5, 5),
    criterion(5, 7),
    criterion(5, 8),
    criterion(5, 9),
    criterion(5, 10),
];
const LICENSE_CRITERIA: [CriterionId; 11] = [
    criterion(6, 1),
    criterion(6, 2),
    criterion(6, 3),
    criterion(6, 4),
    criterion(6, 5),
    criterion(6, 6),
    criterion(6, 7),
    criterion(6, 8),
    criterion(6, 9),
    criterion(6, 10),
    criterion(6, 11),
];
const RECORD_CRITERIA: [CriterionId; 12] = [
    criterion(7, 1),
    criterion(7, 2),
    criterion(7, 3),
    criterion(7, 4),
    criterion(7, 5),
    criterion(7, 6),
    criterion(7, 7),
    criterion(7, 8),
    criterion(7, 9),
    criterion(7, 10),
    criterion(7, 11),
    criterion(7, 12),
];

pub(crate) const ALL_GATES: [GateId; 8] = [
    GateId::Artifact,
    GateId::Platform,
    GateId::Api,
    GateId::Abi,
    GateId::Popup,
    GateId::Window,
    GateId::License,
    GateId::Record,
];
