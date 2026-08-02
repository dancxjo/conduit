use conduit_runtime::{
    BoundedClosingCollector, ClosingFlowEvent, CollectError, CollectLimits, CurrentChanges,
    CurrentUpdateRequest, CurrentValueMutationAuthorizer, EachClosingFlow, hold_current,
    sample_current,
};

struct Admit;

impl CurrentValueMutationAuthorizer for Admit {
    type Error = ();

    fn authorize(&mut self, _request: CurrentUpdateRequest) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn flow_each_preserves_list_order_and_emits_one_normal_close() {
    let mut each = EachClosingFlow::new(vec![1_u8, 2, 3]);
    assert_eq!(each.next_event(), Some(ClosingFlowEvent::Item(1)));
    assert_eq!(each.next_event(), Some(ClosingFlowEvent::Item(2)));
    assert_eq!(each.next_event(), Some(ClosingFlowEvent::Item(3)));
    assert_eq!(each.next_event(), Some(ClosingFlowEvent::Closed));
    assert_eq!(each.next_event(), None);
}

#[test]
fn flow_collect_requires_normal_close_and_enforces_both_bounds() {
    let limits = CollectLimits {
        maximum_items: 2,
        maximum_bytes: 5,
    };
    let mut collector = BoundedClosingCollector::new(limits).unwrap();
    collector.accept(ClosingFlowEvent::Item("ab"), 2).unwrap();
    collector.accept(ClosingFlowEvent::Item("cde"), 3).unwrap();
    let rejection = collector
        .accept(ClosingFlowEvent::Item("f"), 1)
        .unwrap_err();
    assert_eq!(rejection.reason, CollectError::ItemLimitExceeded);
    assert_eq!(collector.accepted_items(), 2);
    assert_eq!(collector.accepted_bytes(), 5);

    collector.accept(ClosingFlowEvent::Closed, 0).unwrap();
    assert_eq!(collector.into_list().unwrap(), vec!["ab", "cde"]);

    let mut incomplete = BoundedClosingCollector::<u8>::new(limits).unwrap();
    incomplete.accept(ClosingFlowEvent::Item(1), 1).unwrap();
    assert_eq!(
        incomplete.into_list(),
        Err(CollectError::NormalCloseRequired)
    );
}

#[test]
fn flow_collect_rejects_byte_overflow_without_mutating_resident_items() {
    let mut collector = BoundedClosingCollector::new(CollectLimits {
        maximum_items: 4,
        maximum_bytes: 3,
    })
    .unwrap();
    collector.accept(ClosingFlowEvent::Item("ab"), 2).unwrap();
    let rejection = collector
        .accept(ClosingFlowEvent::Item("cd"), 2)
        .unwrap_err();
    assert_eq!(rejection.reason, CollectError::ByteLimitExceeded);
    assert_eq!(collector.accepted_items(), 1);
    assert_eq!(collector.accepted_bytes(), 2);
}

#[test]
fn sample_changes_and_hold_keep_their_distinct_temporal_boundaries() {
    let mut held = hold_current(10_u8);
    assert_eq!(sample_current(&held).value, &10);

    let mut changes = CurrentChanges::new(&held);
    assert_eq!(changes.poll(&held), Ok(None));

    held.replace(11, &mut Admit).unwrap();
    let changed = changes.poll(&held).unwrap().unwrap();
    assert_eq!(changed.value, &11);
    assert_eq!(changes.poll(&held), Ok(None));
}
