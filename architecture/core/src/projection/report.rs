//! Admission freezes one borrowed report and computes fidelity from its facts.
use super::*;
use crate::TerminalCategory;

pub struct ProjectionInput<'a, D: ProjectionDomain> {
    pub basis: ProjectionBasis<'a>,
    pub source: &'a D::Source,
    pub target: Option<&'a D::Target>,
    pub facts: &'a [ProjectionFact<'a, D>],
    pub native: &'a [ProjectionNativeFact<'a>],
    pub scores: &'a [ProjectionScore<'a>],
    pub admitted: &'a [ProjectionRealization<'a>],
    pub attempts: &'a [ProjectionAttempt<'a>],
    pub selected_attempt: Option<usize>,
    pub mechanism: ProjectionMechanism,
    pub diagnostics: &'a [ProjectionText<'a>],
}

/// No allocation, retries, planner decisions or execution attestation. A Sign may
/// establish emission separately. The caller retains the exact source/artifact.
pub struct ProjectionReport<'a, D: ProjectionDomain> {
    input: ProjectionInput<'a, D>,
    source_contract: ProjectionText<'a>,
    target_contract: ProjectionText<'a>,
    policy: ProjectionText<'a>,
    summary: ProjectionSummary,
}
impl<'a, D: ProjectionDomain> ProjectionReport<'a, D> {
    pub fn new<P: ProjectionPolicy<D>>(
        domain: &'a D,
        policy: &'a P,
        input: ProjectionInput<'a, D>,
    ) -> Result<Self, ProjectionRefusal> {
        use ProjectionRefusal::*;
        if input.facts.len() > MAX_PROJECTION_FACTS {
            return Err(FactBound);
        }
        if input.native.len() > MAX_PROJECTION_NATIVE_FACTS
            || input
                .native
                .iter()
                .any(|n| n.bytes.is_empty() || n.bytes.len() > MAX_PROJECTION_NATIVE_BYTES)
        {
            return Err(NativeBound);
        }
        if input.scores.len() > MAX_PROJECTION_SCORES {
            return Err(ScoreBound);
        }
        if input.attempts.len() > MAX_PROJECTION_ATTEMPTS
            || input.admitted.len() > MAX_PROJECTION_ATTEMPTS
        {
            return Err(AttemptBound);
        }
        if input.diagnostics.len() > MAX_PROJECTION_DIAGNOSTICS {
            return Err(DiagnosticBound);
        }
        if let Some(boundary) = input.basis.boundary {
            ProjectionText::new(boundary.expanded_form.as_str())?;
        }
        if input.target.is_some() != input.basis.target.is_some() {
            return Err(TargetIdentity);
        }
        for (i, fact) in input.facts.iter().enumerate() {
            if input.facts[..i]
                .iter()
                .any(|prior| prior.obligation() == fact.obligation())
            {
                return Err(DuplicateIdentity);
            }
            if let ProjectionFact::Lost {
                class, native_fact, ..
            } = fact
            {
                if (*class == ProjectionLoss::Unrecognized && native_fact.is_none())
                    || native_fact.is_some_and(|id| !input.native.iter().any(|n| n.identity == id))
                {
                    return Err(UnknownNativeFact);
                }
            }
        }
        for (i, native) in input.native.iter().enumerate() {
            if input.native[..i]
                .iter()
                .any(|n| n.identity == native.identity)
            {
                return Err(DuplicateIdentity);
            }
        }
        if input.scores.iter().any(|score| {
            !input
                .native
                .iter()
                .any(|n| n.identity == score.native_fact())
        }) {
            return Err(UnknownNativeFact);
        }
        for score in input.scores {
            let native = input
                .native
                .iter()
                .find(|n| n.identity == score.native_fact())
                .ok_or(UnknownNativeFact)?;
            let original = match *score {
                ProjectionScore::Native { score, .. } => score,
                ProjectionScore::Normalized {
                    original,
                    projected,
                    policy: normalization,
                    ..
                } => {
                    if !policy.permits_normalization(original, projected, normalization) {
                        return Err(PolicyMismatch);
                    }
                    original
                }
            };
            if original.producer().as_str() != native.provider.as_str() {
                return Err(Domain);
            }
        }
        for (i, realization) in input.admitted.iter().enumerate() {
            if input.admitted[..i].contains(realization) {
                return Err(DuplicateIdentity);
            }
            for id in [
                realization.plan.as_str(),
                realization.host.as_str(),
                realization.boot.as_str(),
                realization.back.as_str(),
                realization.artifact.as_str(),
            ] {
                ProjectionText::new(id)?;
            }
        }
        for (i, attempt) in input.attempts.iter().enumerate() {
            if !input.admitted.contains(&attempt.realization) {
                return Err(Realization);
            }
            if i > 0 && attempt.fallback_reason.is_none() {
                return Err(FallbackReason);
            }
            validate_terminal(attempt.outcome)?;
            // A completed attempt terminates the mechanism chain. Reports cannot
            // hide further trials after selection or duplicate retries.
            if input.attempts[..i].iter().any(|a| {
                a.realization == attempt.realization || a.outcome == ProjectionMechanism::Completed
            }) {
                return Err(AttemptOutcome);
            }
        }
        validate_terminal(input.mechanism)?;
        if let Some(index) = input.selected_attempt {
            if index + 1 != input.attempts.len()
                || input
                    .attempts
                    .get(index)
                    .is_none_or(|a| a.outcome != ProjectionMechanism::Completed)
                || input.mechanism != ProjectionMechanism::Completed
            {
                return Err(AttemptOutcome);
            }
        } else if input
            .attempts
            .iter()
            .any(|a| a.outcome == ProjectionMechanism::Completed)
        {
            return Err(AttemptOutcome);
        }
        if let Some(last) = input.attempts.last() {
            if last.outcome != input.mechanism {
                return Err(AttemptOutcome);
            }
        }
        if input.mechanism != ProjectionMechanism::Completed && input.target.is_some() {
            return Err(AttemptOutcome);
        }
        if input.mechanism != ProjectionMechanism::Completed
            && input.facts.iter().any(|fact| {
                matches!(
                    fact,
                    ProjectionFact::Preserved { .. } | ProjectionFact::Transformed { .. }
                )
            })
        {
            return Err(AttemptOutcome);
        }
        if !domain.validate(
            input.source,
            input.target,
            input.facts,
            input.native,
            input.scores,
            input.mechanism,
        ) {
            return Err(Domain);
        }
        let mut summary = ProjectionSummary {
            disposition: ProjectionDisposition::Insufficient,
            preserved: 0,
            transformed: 0,
            losses: [0; PROJECTION_LOSS_CLASSES],
        };
        for fact in input.facts {
            match fact {
                ProjectionFact::Preserved { .. } => summary.preserved += 1,
                ProjectionFact::Transformed { .. } => summary.transformed += 1,
                ProjectionFact::Lost { class, .. } => summary.losses[*class as usize] += 1,
            }
        }
        summary.disposition = match input.mechanism {
            ProjectionMechanism::Completed => match input.target {
                None => ProjectionDisposition::Insufficient,
                Some(_) if summary.losses.iter().all(|n| *n == 0) => {
                    ProjectionDisposition::Completed(ProjectionFidelity::Exact)
                }
                Some(target) if policy.permits(input.source, target, input.facts) => {
                    ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
                }
                Some(_) => ProjectionDisposition::Insufficient,
            },
            ProjectionMechanism::Refused(info) => ProjectionDisposition::Refused(info),
            ProjectionMechanism::Failed(info) => ProjectionDisposition::Failed(info),
            ProjectionMechanism::Cancelled(info) => ProjectionDisposition::Cancelled(info),
        };
        Ok(Self {
            source_contract: domain.source_contract(),
            target_contract: domain.target_contract(),
            policy: policy.identity(),
            input,
            summary,
        })
    }
    pub fn basis(&self) -> ProjectionBasis<'a> {
        self.input.basis
    }
    pub fn source_contract(&self) -> ProjectionText<'a> {
        self.source_contract
    }
    pub fn target_contract(&self) -> ProjectionText<'a> {
        self.target_contract
    }
    pub fn policy(&self) -> ProjectionText<'a> {
        self.policy
    }
    pub fn summary(&self) -> ProjectionSummary {
        self.summary
    }
    pub fn source(&self) -> &'a D::Source {
        self.input.source
    }
    /// Inspection may see a partial artifact; this is not permission to consume it.
    pub fn inspect_target(&self) -> Option<&'a D::Target> {
        self.input.target
    }
    pub fn require_exact(&self) -> Result<&'a D::Target, ProjectionRefusal> {
        if self.summary.disposition != ProjectionDisposition::Completed(ProjectionFidelity::Exact) {
            return Err(ProjectionRefusal::ConsumerRequiresExact);
        }
        self.input
            .target
            .ok_or(ProjectionRefusal::ConsumerRequiresExact)
    }
    pub fn require_policy(
        &self,
        policy: ProjectionText<'_>,
    ) -> Result<&'a D::Target, ProjectionRefusal> {
        if policy != self.policy {
            return Err(ProjectionRefusal::PolicyMismatch);
        }
        if !matches!(
            self.summary.disposition,
            ProjectionDisposition::Completed(_)
        ) {
            return Err(ProjectionRefusal::ConsumerRequiresExact);
        }
        self.input
            .target
            .ok_or(ProjectionRefusal::ConsumerRequiresExact)
    }
    pub fn facts(&self) -> &'a [ProjectionFact<'a, D>] {
        self.input.facts
    }
    pub fn native(&self) -> &'a [ProjectionNativeFact<'a>] {
        self.input.native
    }
    pub fn scores(&self) -> &'a [ProjectionScore<'a>] {
        self.input.scores
    }
    pub fn attempts(&self) -> &'a [ProjectionAttempt<'a>] {
        self.input.attempts
    }
    pub fn selected_attempt(&self) -> Option<&'a ProjectionAttempt<'a>> {
        self.input.selected_attempt.map(|i| &self.input.attempts[i])
    }
    pub fn diagnostics(&self) -> &'a [ProjectionText<'a>] {
        self.input.diagnostics
    }
}
fn validate_terminal(outcome: ProjectionMechanism) -> Result<(), ProjectionRefusal> {
    let valid = match outcome {
        ProjectionMechanism::Completed => true,
        ProjectionMechanism::Refused(info) => matches!(
            info.category(),
            TerminalCategory::RefusalOrAdmission
                | TerminalCategory::UnavailableRealization
                | TerminalCategory::StaleOrInvalidated
        ),
        ProjectionMechanism::Cancelled(info) => info.category() == TerminalCategory::Cancelled,
        ProjectionMechanism::Failed(info) => matches!(
            info.category(),
            TerminalCategory::ExecutionFault
                | TerminalCategory::Deadline
                | TerminalCategory::ProviderOrResourceLoss
                | TerminalCategory::DomainOwned
        ),
    };
    if valid {
        Ok(())
    } else {
        Err(ProjectionRefusal::TerminalCategory)
    }
}
