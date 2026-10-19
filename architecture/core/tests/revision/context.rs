use super::*;

struct Referencing(Frame);
impl RevisionDomain for Referencing {
    type Delta = TrackDelta;
    type Cursor = Frame;
    fn contract(&self) -> RevisionText<'_> {
        Tracking.contract()
    }
    fn validate_delta(&self, role: RevisionDeltaRole, delta: &TrackDelta) -> bool {
        Tracking.validate_delta(role, delta)
    }
    fn validate_cursor(&self, cursor: Frame) -> bool {
        Tracking.validate_cursor(cursor)
    }
    fn region(&self, delta: &TrackDelta) -> (Frame, Frame) {
        Tracking.region(delta)
    }
    fn observation_end(&self, _: &TrackDelta) -> Frame {
        self.0
    }
    fn distance(&self, start: Frame, end: Frame) -> Option<u64> {
        Tracking.distance(start, end)
    }
}
#[test]
fn referenced_context_must_enclose_change_and_fit_domain_and_window() {
    let change = delta(0, 2, 1);
    for observed in [Frame(1), Frame(33)] {
        let domain = Referencing(observed);
        assert!(matches!(
            RevisionEvent::new(
                &domain,
                reference(1, "invalid"),
                &EVIDENCE,
                RevisionChange::Proposed { delta: &change }
            ),
            Err(RevisionRefusal::Domain)
        ));
    }
    let domain = Referencing(Frame(5));
    let proposed = RevisionEvent::new(
        &domain,
        reference(1, "wide-context"),
        &EVIDENCE,
        RevisionChange::Proposed { delta: &change },
    )
    .unwrap();
    let mut journal = RevisionJournal::new(&domain, context(), Frame(0), limits()).unwrap();
    assert_eq!(
        journal.append(&proposed),
        Err(RevisionRefusal::RevisableRegionBound)
    );
    assert_eq!(journal.history().count(), 0);
    assert_eq!(journal.frontiers().observed_through, Frame(0));
    assert_eq!(journal.frontiers().committed, Frame(0));
}
