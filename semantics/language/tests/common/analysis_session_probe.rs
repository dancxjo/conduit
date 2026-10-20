//! Actual Source-prepared receipt publication; no future receipt-generation claim.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
use conduit_core::revision::*;
use conduit_language::{
    analysis_session::{AnalysisSession, AnalysisSessionRefusal},
    dependency_commitment::PreparedCommittedDependency,
    revision::*,
};
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
pub fn append_and_current<'a>(
    session: &mut AnalysisSession<'a>,
    commit: &'a RevisionEvent<'a, DependencyRevisions<'a>>,
    proposed: RevisionReference<'a>,
    receipt: &PreparedCommittedDependency,
) {
    let before = session.frontiers();
    let count = session.history().count();
    let (_, observation) = allocation_probe::observe(|| {
        assert!(matches!(
            session.append(commit, Some(receipt), true, false),
            Err(AnalysisSessionRefusal::Cancelled)
        ));
        assert!(matches!(
            session.append(commit, Some(receipt), false, true),
            Err(AnalysisSessionRefusal::Pressure)
        ));
        assert_eq!(session.frontiers(), before);
        assert_eq!(session.history().count(), count);
        session.append(commit, Some(receipt), false, false).unwrap();
        let current = session.current(proposed, receipt).unwrap();
        assert!(core::ptr::eq(current.receipt(), receipt));
        assert_eq!(
            current.frontier().0,
            *receipt.commitment().result().frontier().committed()
        );
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
    assert_eq!(observation.deallocations, 0);
}
