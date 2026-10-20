//! One authoritative bounded prosody ledger, composing the shared revision law.
//! It owns current prosody, not the upstream parser or physical playback device.
use crate::{
    prosody::PreparedFallbackProsody, prosody_commitment::PreparedCommittedProsody,
    revision::TokenFrontier, *,
};
use conduit_core::revision::*;
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};

/// The variant is evidence classification, not a successful parse flag.
pub enum ProsodyPlan<'a> {
    CommittedDiscourse(&'a PreparedCommittedProsody),
    HeuristicFallback(&'a PreparedFallbackProsody),
}
impl ProsodyPlan<'_> {
    pub fn source(&self) -> &LanguageTextRevision {
        match self {
            Self::CommittedDiscourse(p) => p.admission().rich().source(),
            Self::HeuristicFallback(p) => p.requested().source(),
        }
    }
    pub fn token(&self) -> &LinguisticTokenIdentity {
        match self {
            Self::CommittedDiscourse(p) => p.admission().rich().token().identity(),
            Self::HeuristicFallback(p) => p.requested().token().identity(),
        }
    }
}
pub struct ProsodyRevisions<'a> {
    source: &'a LanguageTextRevision,
    tokens: u64,
}
impl<'a> ProsodyRevisions<'a> {
    pub fn new(source: &'a LanguageTextRevision, tokens: u64) -> Result<Self, RevisionRefusal> {
        if tokens == 0 || tokens > 4 {
            return Err(RevisionRefusal::Domain);
        }
        Ok(Self { source, tokens })
    }
}
impl<'a> RevisionDomain for ProsodyRevisions<'a> {
    type Delta = ProsodyPlan<'a>;
    type Cursor = TokenFrontier;
    fn contract(&self) -> RevisionText<'_> {
        RevisionText::new("language/prosody-revision@1").unwrap()
    }
    fn validate_delta(&self, _role: RevisionDeltaRole, delta: &Self::Delta) -> bool {
        delta.source() == self.source && *delta.token().ordinal() < self.tokens
    }
    fn validate_cursor(&self, cursor: Self::Cursor) -> bool {
        cursor.0 <= self.tokens
    }
    fn region(&self, delta: &Self::Delta) -> (Self::Cursor, Self::Cursor) {
        // This finite plan owns exactly one occurrence's intent. Later token
        // plans remain revisable while the committed earlier interval is kept.
        (
            TokenFrontier(*delta.token().ordinal()),
            TokenFrontier(*delta.token().ordinal() + 1),
        )
    }
    fn distance(&self, start: Self::Cursor, end: Self::Cursor) -> Option<u64> {
        end.0.checked_sub(start.0)
    }
}
#[derive(Debug)]
pub enum ProsodySessionRefusal {
    Revision(RevisionRefusal),
    Cancelled,
    Pressure,
    BeyondStable,
    Stale,
    Program,
    AnalysisAuthority,
}
/// No caller-supplied `current` Boolean: the journal is this session's authority.
/// It does not grant installed upstream currentness or claim physical playback.
pub struct ProsodySession<'a> {
    journal: RevisionJournal<'a, ProsodyRevisions<'a>>,
}
impl<'a> ProsodySession<'a> {
    pub fn new(
        domain: &'a ProsodyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<Self, RevisionRefusal> {
        Ok(Self {
            journal: RevisionJournal::new(domain, context, TokenFrontier(0), limits)?,
        })
    }
    /// Rich publication must compose the current owning analysis session. A
    /// preparation receipt alone cannot assert freshness. Heuristic fallback is
    /// explicitly separate and must not be advertised as parser-backed output.
    pub fn append_current(
        &mut self,
        event: &'a RevisionEvent<'a, ProsodyRevisions<'a>>,
        authority: &crate::analysis_session::CurrentCommittedDependency<'_, '_>,
        cancelled: bool,
        pressure: bool,
    ) -> Result<(), ProsodySessionRefusal> {
        let plan = self.event_plan(event);
        if let Some(ProsodyPlan::CommittedDiscourse(plan)) = plan {
            if plan.dependency().arc() != authority.receipt().arc()
                || plan.dependency().admission() != authority.receipt().admission()
                || plan.dependency().lexical() != authority.receipt().lexical()
            {
                return Err(ProsodySessionRefusal::AnalysisAuthority);
            }
        }
        self.append_admitted(event, cancelled, pressure)
    }
    /// Only heuristic proposals may enter without current analysis authority.
    pub fn append(
        &mut self,
        event: &'a RevisionEvent<'a, ProsodyRevisions<'a>>,
        cancelled: bool,
        pressure: bool,
    ) -> Result<(), ProsodySessionRefusal> {
        if matches!(
            self.event_plan(event),
            Some(ProsodyPlan::CommittedDiscourse(_))
        ) {
            return Err(ProsodySessionRefusal::AnalysisAuthority);
        }
        self.append_admitted(event, cancelled, pressure)
    }
    fn event_plan(
        &self,
        event: &'a RevisionEvent<'a, ProsodyRevisions<'a>>,
    ) -> Option<&'a ProsodyPlan<'a>> {
        match event.change() {
            RevisionChange::Proposed { delta } | RevisionChange::Revised { delta, .. } => {
                Some(*delta)
            }
            RevisionChange::Stable { revision, .. }
            | RevisionChange::Committed { revision, .. } => {
                self.journal
                    .history()
                    .find(|e| e.reference() == *revision)
                    .and_then(|e| match e.change() {
                        RevisionChange::Proposed { delta }
                        | RevisionChange::Revised { delta, .. } => Some(*delta),
                        _ => None,
                    })
            }
            _ => None,
        }
    }
    fn append_admitted(
        &mut self,
        event: &'a RevisionEvent<'a, ProsodyRevisions<'a>>,
        cancelled: bool,
        pressure: bool,
    ) -> Result<(), ProsodySessionRefusal> {
        let frontier = self.journal.frontiers();
        let through = match event.change() {
            RevisionChange::Committed { through, .. } => through.0,
            _ => frontier.committed.0,
        };
        let stable = frontier
            .stable_through
            .map_or(frontier.committed.0, |v| v.0);
        let query = LanguageProsodyFrontierRequest::new(
            cancelled,
            frontier.committed.0,
            pressure,
            stable,
            through,
        )
        .map_err(|_| ProsodySessionRefusal::Program)?;
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/prosody_frontier_program.hex"
        )))
        .map_err(|_| ProsodySessionRefusal::Program)?;
        let output = program
            .evaluate(&query.encode().map_err(|_| ProsodySessionRefusal::Program)?)
            .map_err(|_| ProsodySessionRefusal::Program)?;
        if !*LanguageProsodyFrontierResult::decode(&output)
            .map_err(|_| ProsodySessionRefusal::Program)?
            .accepted()
        {
            return Err(if cancelled {
                ProsodySessionRefusal::Cancelled
            } else if pressure {
                ProsodySessionRefusal::Pressure
            } else {
                ProsodySessionRefusal::BeyondStable
            });
        }
        self.journal
            .append(event)
            .map_err(ProsodySessionRefusal::Revision)
    }
    pub fn require_current(
        &self,
        reference: RevisionReference<'a>,
    ) -> Result<(), ProsodySessionRefusal> {
        if self.journal.current_proposal() != Some(reference) {
            return Err(ProsodySessionRefusal::Stale);
        }
        Ok(())
    }
    pub fn frontiers(&self) -> RevisionFrontiers<TokenFrontier> {
        self.journal.frontiers()
    }
    pub fn history(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a RevisionEvent<'a, ProsodyRevisions<'a>>> + '_ {
        self.journal.history()
    }
}
