//! Closed finite target registry used by the fixed Session orchestration.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_execution::ParserSessionEntry as Entry,
    parser_session_fixed_ingress::{ParserSessionExecutor, PreparedParserFixedIngress},
    parser_session_mixed_custody::PreparedParserMixedCustody,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_stage::ParserSessionTargets,
};
use alloc::vec::Vec;

// Completion is the named Native observation of the original Boolean entry.
// Feature/index/score wrapper owners belong to the separately admitted mixed
// owner. They cannot be replaced by a caller-selected Source port here.
const REQUIRED: &[Entry] = &[
    Entry::Availability,
    Entry::Completion,
    Entry::IndependentBranch,
    Entry::IndependentCommit,
    Entry::IndependentCommitRebase,
    Entry::IndependentCommitRebaseSets,
    Entry::IndependentMask,
    Entry::JointBranch,
    Entry::Commit,
    Entry::JointConsensus,
    Entry::Expansion,
    Entry::Rebase,
    Entry::Merge,
    Entry::JointScoreBand1000,
    Entry::StableFact,
    Entry::LegalMask,
    Entry::ProtectedOriginEdge,
    Entry::Initialize,
    Entry::ProtectedInsert,
    Entry::ProtectedRebase,
    Entry::ProtectionForestProjection,
    Entry::RetainedCommitAnchor,
    Entry::RevisionReset,
    Entry::ScoreProposal,
    Entry::Seed,
    Entry::Transition,
    Entry::V2Pos,
    Entry::WaitState,
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegistryRefusal {
    MissingOrDuplicate,
    Closed,
}

pub(crate) struct ParserSessionTargetRegistry<
    E: ParserSessionExecutor,
    S: ParserCanonicalSourceExecutor,
    N: ParserNumericExecutor,
> {
    ports: Vec<PreparedParserFixedIngress<E>>,
    mixed: PreparedParserMixedCustody<S, N>,
    cancelled: bool,
}
impl<E: ParserSessionExecutor, S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>
    ParserSessionTargetRegistry<E, S, N>
{
    /// The fixed Session factory admits each complete Source/Native/Plan owner
    /// and the aggregate resource reservation before passing ownership here.
    /// Registry assembly performs no allocation and permits no optional port.
    pub(crate) fn from_prepared(
        mut ports: Vec<PreparedParserFixedIngress<E>>,
        mut mixed: PreparedParserMixedCustody<S, N>,
    ) -> Result<Self, RegistryRefusal> {
        if !complete_entries(ports.iter().map(|port| port.entry())) {
            for port in &mut ports {
                port.cancel();
            }
            mixed.cancel();
            return Err(RegistryRefusal::MissingOrDuplicate);
        }
        Ok(Self {
            ports,
            mixed,
            cancelled: false,
        })
    }
    pub(crate) fn port(
        &mut self,
        entry: Entry,
    ) -> Result<&mut PreparedParserFixedIngress<E>, RegistryRefusal> {
        if self.cancelled {
            return Err(RegistryRefusal::Closed);
        }
        self.ports
            .iter_mut()
            .find(|port| port.entry() == entry)
            .ok_or(RegistryRefusal::MissingOrDuplicate)
    }
    pub(crate) fn mixed(
        &mut self,
    ) -> Result<&mut PreparedParserMixedCustody<S, N>, RegistryRefusal> {
        if self.cancelled {
            return Err(RegistryRefusal::Closed);
        }
        Ok(&mut self.mixed)
    }
}
fn complete_entries(entries: impl Iterator<Item = Entry>) -> bool {
    let mut seen = [false; 28];
    let mut count = 0;
    for entry in entries {
        let Some(index) = REQUIRED.iter().position(|required| *required == entry) else {
            return false;
        };
        if seen[index] {
            return false;
        }
        seen[index] = true;
        count += 1;
    }
    count == REQUIRED.len() && seen.iter().all(|present| *present)
}
impl<E: ParserSessionExecutor, S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>
    ParserSessionTargets for ParserSessionTargetRegistry<E, S, N>
{
    fn cancel_all(&mut self) {
        if !self.cancelled {
            self.cancelled = true;
            for port in &mut self.ports {
                port.cancel();
            }
            self.mixed.cancel();
        }
    }
}
impl<E: ParserSessionExecutor, S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> Drop
    for ParserSessionTargetRegistry<E, S, N>
{
    fn drop(&mut self) {
        self.cancel_all();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_requires_every_fixed_port_once() {
        assert!(complete_entries(REQUIRED.iter().copied()));
        assert!(!complete_entries(
            REQUIRED[..REQUIRED.len() - 1].iter().copied()
        ));
        assert!(!complete_entries(
            REQUIRED
                .iter()
                .copied()
                .chain(core::iter::once(Entry::Seed))
        ));
        assert!(!complete_entries(
            REQUIRED
                .iter()
                .copied()
                .chain(core::iter::once(Entry::DecodeComplete))
        ));
        let mut duplicate = REQUIRED.to_vec();
        duplicate[0] = duplicate[1];
        assert!(!complete_entries(duplicate.into_iter()));
    }
}
