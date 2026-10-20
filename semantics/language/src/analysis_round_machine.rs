//! The existing bounded round ordering, with canonical registers instead of Native clones.
use super::{material::*, storage::*, *};
use alloc::vec::Vec;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Seed,
    SeedAdmit,
    Slot,
    Complete,
    ProbeProposal,
    ProbeTransition,
    Mask,
    Choice,
    Branch,
    BranchAdmit,
    Score,
    Features,
    Numerical,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    LearnedRank,
    Rank,
    RankedProposal,
    Transition,
    Expand,
    CandidateAdmit,
    Merge,
    NextRank,
    Done,
}
#[derive(Clone, Copy)]
pub(super) enum Work {
    Source(&'static str, &'static str, Option<&'static str>),
    Native(&'static str),
    Numerical,
    Complete,
}
pub(super) struct Machine {
    phase: Phase,
    slot: usize,
    choice: u64,
    probe: usize,
    ranked: usize,
    complete_merge: bool,
    pub identity: u64,
    pub invocation: u64,
    pub mode: AnalysisRoundMode,
    pub prior_beam: Vec<u8>,
    pub lexical: Vec<u8>,
    pub initial_raw: Vec<u8>,
    pub query: Vec<u8>,
    pub next: Vec<u8>,
    prior: Vec<u8>,
    branch: Vec<u8>,
    mask: Vec<u8>,
    rank: Vec<u8>,
    proposal: Vec<u8>,
    result: Vec<u8>,
    raw: Vec<u8>,
    candidate: Vec<u8>,
    numeric: Vec<u8>,
    scalar: Vec<u8>,
    slots: [Vec<u8>; 4],
    probes: [Vec<u8>; 5],
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    zero: Vec<u8>,
    zero_i64: Vec<u8>,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    false_value: Vec<u8>,
    relation: Vec<u8>,
    pub features: Vec<u8>,
    pub feature_query: Vec<u8>,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub numerical_input: Vec<u8>,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub numerical_scores: Vec<u8>,
}
impl Machine {
    pub fn prepare(
        owner: &AnalysisEpochOwner<'_>,
        mode: AnalysisRoundMode,
        invocation: u64,
    ) -> R<Self> {
        let lexical = LanguageParserAvailableLexical::new(
            owner.lexical().tape().clone(),
            *owner.beam().candidate0().parser().state().token_count(),
        )
        .map_err(|_| CursorRefusal::Preparation)?
        .encode()
        .map_err(|_| CursorRefusal::Preparation)?;
        let prior_beam = owner
            .beam()
            .clone()
            .encode()
            .map_err(|_| CursorRefusal::Preparation)?;
        let initial_raw = super::super::raw_beam(owner.beam())
            .map_err(|_| CursorRefusal::Preparation)?
            .encode()
            .map_err(|_| CursorRefusal::Preparation)?;
        if [lexical.len(), prior_beam.len(), initial_raw.len()]
            .into_iter()
            .any(|v| v > BYTES)
        {
            return Err(CursorRefusal::Bounds);
        }
        Ok(Self {
            phase: Phase::Seed,
            slot: 0,
            choice: 0,
            probe: 0,
            ranked: 0,
            complete_merge: false,
            identity: owner.identity_cursor,
            invocation,
            mode,
            lexical,
            prior_beam,
            initial_raw,
            query: buffer(),
            next: buffer(),
            prior: buffer(),
            branch: buffer(),
            mask: buffer(),
            rank: buffer(),
            proposal: buffer(),
            result: buffer(),
            raw: buffer(),
            candidate: buffer(),
            numeric: buffer(),
            scalar: buffer(),
            slots: core::array::from_fn(|_| buffer()),
            probes: core::array::from_fn(|_| buffer()),
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "ptr",
                target_has_atomic = "8"
            ))]
            zero: scalar("value/u64", &0u64)?,
            zero_i64: scalar("value/i64", &0i64)?,
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "ptr",
                target_has_atomic = "8"
            ))]
            false_value: scalar("value/bool", &false)?,
            relation: LanguageParserRelation::new(
                LanguageUniversalDependencyRelation::Dep,
                LanguageParserSubtype::new(String::new())
                    .map_err(|_| CursorRefusal::Preparation)?,
            )
            .map_err(|_| CursorRefusal::Preparation)?
            .encode()
            .map_err(|_| CursorRefusal::Preparation)?,
            features: buffer(),
            feature_query: buffer(),
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "ptr",
                target_has_atomic = "8"
            ))]
            numerical_input: buffer(),
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "ptr",
                target_has_atomic = "8"
            ))]
            numerical_scores: buffer(),
        })
    }
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub fn branch_identity(&self) -> (usize, u64) {
        (self.slot, self.choice)
    }
    fn alternative_count(&self) -> R<u64> {
        Ok(
            if number(member(&self.prior, &["parser", "state", "unread"])?)?
                < number(member(&self.lexical, &["token_count"])?)?
            {
                4
            } else {
                1
            },
        )
    }
    fn next_choice(&mut self) -> R<()> {
        self.choice += 1;
        if self.choice >= self.alternative_count()? {
            self.slot += 1;
            self.phase = Phase::Slot;
        } else {
            self.phase = Phase::Choice;
        }
        Ok(())
    }
    pub fn native_complete(&mut self) -> R<()> {
        use Phase::*;
        match self.phase {
            SeedAdmit => {
                put(&mut self.next, &self.query)?;
                self.phase = Slot;
            }
            BranchAdmit => {
                put(&mut self.branch, &self.query)?;
                self.phase = Score;
            }
            CandidateAdmit => {
                put(&mut self.candidate, &self.query)?;
                self.complete_merge = false;
                self.phase = Merge;
            }
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "ptr",
                target_has_atomic = "8"
            ))]
            LearnedRank => {
                put(&mut self.rank, &self.query)?;
                self.phase = Rank;
            }
            _ => return Err(CursorRefusal::Lifecycle),
        }
        Ok(())
    }
    pub fn receive(&mut self, bytes: &[u8]) -> R<()> {
        use Phase::*;
        match self.phase {
            Seed => {
                put(&mut self.raw, bytes)?;
                self.phase = SeedAdmit;
            }
            Complete => match bytes {
                [1] => {
                    put(&mut self.candidate, &self.prior)?;
                    self.complete_merge = true;
                    self.phase = Merge;
                }
                [0] => self.phase = ProbeProposal,
                _ => return Err(CursorRefusal::Canonical),
            },
            ProbeProposal => {
                put(&mut self.proposal, bytes)?;
                self.phase = ProbeTransition;
            }
            RankedProposal => {
                put(&mut self.proposal, bytes)?;
                self.phase = Transition;
            }
            ProbeTransition => {
                put(&mut self.probes[self.probe], bytes)?;
                self.probe += 1;
                self.phase = if self.probe == 5 { Mask } else { ProbeProposal };
            }
            Mask => {
                put(&mut self.mask, bytes)?;
                self.phase = Choice;
            }
            Branch => {
                put(&mut self.raw, bytes)?;
                if boolean(member(bytes, &["accepted"])?)? {
                    self.phase = BranchAdmit;
                } else {
                    self.next_choice()?;
                }
            }
            Score => {
                put(&mut self.rank, bytes)?;
                self.phase = Rank;
            }
            Features => {
                put(&mut self.features, bytes)?;
                self.phase = Numerical;
            }
            Rank => {
                put(&mut self.rank, bytes)?;
                if boolean(member(bytes, &["found"])?)? {
                    self.phase = RankedProposal;
                } else {
                    self.next_choice()?;
                }
            }
            Transition => {
                put(&mut self.result, bytes)?;
                self.identity = self.identity.checked_add(1).ok_or(CursorRefusal::Bounds)?;
                self.phase = Expand;
            }
            Expand => {
                put(&mut self.raw, bytes)?;
                if boolean(member(bytes, &["parser", "active"])?)? {
                    self.phase = CandidateAdmit;
                } else {
                    self.phase = NextRank;
                }
            }
            Merge => {
                put(&mut self.next, bytes)?;
                if self.complete_merge {
                    self.slot += 1;
                    self.phase = Slot;
                } else {
                    self.phase = NextRank;
                }
            }
            NextRank => {
                put(&mut self.rank, bytes)?;
                self.ranked += 1;
                if self.ranked == 4 {
                    self.next_choice()?;
                } else {
                    self.phase = Rank;
                }
            }
            _ => return Err(CursorRefusal::Lifecycle),
        }
        Ok(())
    }
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub fn numerical_complete(&mut self) -> R<()> {
        if self.phase != Phase::Numerical {
            return Err(CursorRefusal::Lifecycle);
        }
        self.invocation = self
            .invocation
            .checked_add(1)
            .ok_or(CursorRefusal::Bounds)?;
        self.phase = Phase::LearnedRank;
        Ok(())
    }
}

fn scalar<T: conduit_plot::rust_binding::NativePrimitive>(kind: &str, value: &T) -> R<Vec<u8>> {
    conduit_plot::rust_binding::primitive_into_structured(
        StructuredInfoType::leaf(kind_id(kind)).map_err(|_| CursorRefusal::Preparation)?,
        value,
    )
    .map_err(|_| CursorRefusal::Preparation)?
    .canonical_bytes()
    .map_err(|_| CursorRefusal::Preparation)
}

#[path = "analysis_round_build.rs"]
mod build;

#[path = "analysis_round_bind.rs"]
mod deferred_bind;
