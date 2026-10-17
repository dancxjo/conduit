//! Finite borrowed facts; domains own semantic obligations and typed detail.
use crate::{
    claims::ClaimScore, ArtifactId, BootId, ExpandedPlotId, HostId, ImplementationId, PlanId,
    TerminalInfo,
};

pub const MAX_PROJECTION_FACTS: usize = 64;
pub const MAX_PROJECTION_NATIVE_FACTS: usize = 16;
pub const MAX_PROJECTION_NATIVE_BYTES: usize = 4096;
pub const MAX_PROJECTION_ATTEMPTS: usize = 8;
pub const MAX_PROJECTION_SCORES: usize = 8;
pub const MAX_PROJECTION_DIAGNOSTICS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionText<'a>(&'a str);
impl<'a> ProjectionText<'a> {
    pub fn new(value: &'a str) -> Result<Self, ProjectionRefusal> {
        if value.is_empty() || value.len() > 192 {
            return Err(ProjectionRefusal::TextBound);
        }
        Ok(Self(value))
    }
    pub fn as_str(self) -> &'a str {
        self.0
    }
}

/// Stable generic loss categories; domain details retain their own vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ProjectionLoss {
    Unrepresentable,
    Unrecognized,
    Precision,
    Range,
    Cardinality,
    Ordering,
    TemporalFinality,
    Identity,
    Provenance,
    UnsupportedVariant,
    Truncation,
    Approximation,
}
pub const PROJECTION_LOSS_CLASSES: usize = 12;

pub enum ProjectionFact<'a, D: ProjectionDomain> {
    Preserved {
        obligation: ProjectionText<'a>,
    },
    Transformed {
        obligation: ProjectionText<'a>,
        law: ProjectionText<'a>,
    },
    Lost {
        obligation: ProjectionText<'a>,
        class: ProjectionLoss,
        detail: &'a D::Detail,
        native_fact: Option<ProjectionText<'a>>,
    },
}
impl<D: ProjectionDomain> ProjectionFact<'_, D> {
    pub fn obligation(&self) -> ProjectionText<'_> {
        match self {
            Self::Preserved { obligation }
            | Self::Transformed { obligation, .. }
            | Self::Lost { obligation, .. } => *obligation,
        }
    }
}

/// Reviewed pure bounded validators, not hostile-code confinement. Validate the
/// COMPLETE source obligation inventory, target, typed losses and native evidence.
/// Reject omitted obligations and claimed preservation unsupported by the target.
pub trait ProjectionDomain {
    type Source;
    type Target;
    type Detail;
    fn source_contract(&self) -> ProjectionText<'_>;
    fn target_contract(&self) -> ProjectionText<'_>;
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool
    where
        Self: Sized;
}

/// An exact policy checks the whole inventory, including aggregate loss budgets.
/// No universal best-effort policy is provided.
pub trait ProjectionPolicy<D: ProjectionDomain> {
    fn identity(&self) -> ProjectionText<'_>;
    /// Defaults to refusing score normalization. Implementations must validate
    /// the exact numerical mapping and named policy, retaining the original.
    fn permits_normalization(
        &self,
        _original: crate::claims::ClaimScore<'_>,
        _projected: crate::claims::ClaimScore<'_>,
        _policy: ProjectionText<'_>,
    ) -> bool {
        false
    }
    fn permits(
        &self,
        source: &D::Source,
        target: &D::Target,
        facts: &[ProjectionFact<'_, D>],
    ) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionNativeFact<'a> {
    pub identity: ProjectionText<'a>,
    pub contract: ProjectionText<'a>,
    pub provider: ProjectionText<'a>,
    pub encoding: ProjectionText<'a>,
    pub bytes: &'a [u8],
}

/// Scores keep the existing exact scale/calibration/producer comparison law.
/// Normalization is an explicit claim with an exact policy, not an implicit cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionScore<'a> {
    Native {
        fact: ProjectionText<'a>,
        score: ClaimScore<'a>,
    },
    Normalized {
        fact: ProjectionText<'a>,
        original: ClaimScore<'a>,
        projected: ClaimScore<'a>,
        policy: ProjectionText<'a>,
    },
}
impl<'a> ProjectionScore<'a> {
    pub fn native_fact(self) -> ProjectionText<'a> {
        match self {
            Self::Native { fact, .. } | Self::Normalized { fact, .. } => fact,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionBoundary<'a> {
    pub expanded_form: &'a ExpandedPlotId,
    pub subject: ProjectionText<'a>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionBasis<'a> {
    pub report: ProjectionText<'a>,
    pub source: ProjectionText<'a>,
    pub target: Option<ProjectionText<'a>>,
    pub projector: ProjectionText<'a>,
    pub requested_route: ProjectionText<'a>,
    pub boundary: Option<ProjectionBoundary<'a>>,
}

/// Correlation with already-admitted exact Plan facts; this grants no authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionRealization<'a> {
    pub plan: &'a PlanId,
    pub host: &'a HostId,
    pub boot: &'a BootId,
    pub back: &'a ImplementationId,
    pub artifact: &'a ArtifactId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMechanism {
    Completed,
    Refused(TerminalInfo),
    Failed(TerminalInfo),
    Cancelled(TerminalInfo),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionAttempt<'a> {
    pub realization: ProjectionRealization<'a>,
    pub outcome: ProjectionMechanism,
    /// Exact retained reason for this admitted alternative, required after attempt zero.
    pub fallback_reason: Option<ProjectionText<'a>>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionFidelity {
    Exact,
    PermittedLossy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionDisposition {
    Completed(ProjectionFidelity),
    Insufficient,
    Refused(TerminalInfo),
    Failed(TerminalInfo),
    Cancelled(TerminalInfo),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionRefusal {
    TextBound,
    FactBound,
    NativeBound,
    ScoreBound,
    AttemptBound,
    DiagnosticBound,
    DuplicateIdentity,
    UnknownNativeFact,
    Domain,
    TargetIdentity,
    Realization,
    AttemptOutcome,
    FallbackReason,
    TerminalCategory,
    ConsumerRequiresExact,
    PolicyMismatch,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionSummary {
    pub disposition: ProjectionDisposition,
    pub preserved: usize,
    pub transformed: usize,
    pub losses: [usize; PROJECTION_LOSS_CLASSES],
}
impl ProjectionSummary {
    pub fn truncated(self) -> bool {
        self.losses[ProjectionLoss::Truncation as usize] != 0
    }
}
