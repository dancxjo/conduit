use super::*;
use conduit_core::{
    kind_id, StructuredInfoValue, StructuredInfoValueShape, BOOL_INFO_ID, COUNT_INFO_ID,
    TEXT_INFO_ID,
};
use conduit_kernel::ValueRef;

fn operation(capacity: usize) -> FlowJoinByKeyBack {
    let left = PreparedTuplePairEncoder::new(kind_id(TEXT_INFO_ID), 16, kind_id(COUNT_INFO_ID), 8)
        .unwrap();
    let right =
        PreparedTuplePairEncoder::new(kind_id(TEXT_INFO_ID), 16, kind_id(BOOL_INFO_ID), 1).unwrap();
    let output = PreparedTupleTripleEncoder::new(
        kind_id(TEXT_INFO_ID),
        16,
        kind_id(COUNT_INFO_ID),
        8,
        kind_id(BOOL_INFO_ID),
        1,
    )
    .unwrap();
    let candidate = left.maximum_bytes().max(right.maximum_bytes()) as usize;
    FlowJoinByKeyBack {
        pending: [
            PendingSet::new(capacity, left.maximum_bytes() as usize),
            PendingSet::new(capacity, right.maximum_bytes() as usize),
        ],
        candidate: vec![0; candidate],
        candidate_action: None,
        candidate_closed: None,
        closed: [false; 2],
        codecs: [left, right],
        output: Box::new(output),
        output_staged: false,
        terminal: false,
    }
}

fn keyed(side: usize, key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut codec = if side == 0 {
        PreparedTuplePairEncoder::new(kind_id(TEXT_INFO_ID), 16, kind_id(COUNT_INFO_ID), 8).unwrap()
    } else {
        PreparedTuplePairEncoder::new(kind_id(TEXT_INFO_ID), 16, kind_id(BOOL_INFO_ID), 1).unwrap()
    };
    codec.encode(key, value).unwrap().to_vec()
}

fn reference(slot: u16, bytes: &[u8]) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: bytes.len() as u32,
    }
}

fn step_input(operation: &mut FlowJoinByKeyBack, side: usize, bytes: &[u8]) -> StepOutcome {
    let mut references = [None, None];
    references[side] = Some(reference(side as u16, bytes));
    let mut payloads = [None, None];
    payloads[side] = Some(bytes);
    let mut io = StepIo::test_frame(
        references,
        [false; 2],
        [Some(operation.output.maximum_bytes()), None],
        None,
        32,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame(payloads, None));
    if outcome == StepOutcome::Progress {
        assert!(io.test_consumed(PortId(side as u16)));
    }
    outcome
}

fn commit(operation: &mut FlowJoinByKeyBack) {
    <FlowJoinByKeyBack as StepBack<2>>::step_committed(operation);
}

fn staged_fields(operation: &FlowJoinByKeyBack) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let bytes = <FlowJoinByKeyBack as StepBack<2>>::prepared_output(operation, PortId(0)).unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("joined output must be the canonical triple")
    };
    let leaf = |index: usize| {
        let StructuredInfoValueShape::Leaf(bytes) = fields[index].value().shape() else {
            panic!("joined tuple members are exact semantic leaves")
        };
        bytes.to_vec()
    };
    (leaf(0), leaf(1), leaf(2))
}

#[test]
fn opposite_arrival_completes_one_exact_join() {
    let mut operation = operation(2);
    let left_value = conduit_core::encode_count(7);
    let left = keyed(0, b"alpha", &left_value);
    assert_eq!(step_input(&mut operation, 0, &left), StepOutcome::Progress);
    commit(&mut operation);
    assert_eq!(operation.pending[0].count, 1);

    let right = keyed(1, b"alpha", &[1]);
    assert_eq!(step_input(&mut operation, 1, &right), StepOutcome::Progress);
    assert_eq!(
        staged_fields(&operation),
        (b"alpha".to_vec(), left_value.to_vec(), vec![1])
    );
    commit(&mut operation);
    assert_eq!(operation.pending[0].count, 0);
}

#[test]
fn duplicate_keys_pair_oldest_with_oldest() {
    let mut operation = operation(3);
    for count in [1, 2] {
        let value = conduit_core::encode_count(count);
        let left = keyed(0, b"same", &value);
        assert_eq!(step_input(&mut operation, 0, &left), StepOutcome::Progress);
        commit(&mut operation);
    }
    for expected in [1, 2] {
        let right = keyed(1, b"same", &[1]);
        assert_eq!(step_input(&mut operation, 1, &right), StepOutcome::Progress);
        let (_, left, _) = staged_fields(&operation);
        assert_eq!(left, conduit_core::encode_count(expected));
        commit(&mut operation);
    }
}

#[test]
fn full_unmatched_side_backpressures_without_eviction() {
    let mut operation = operation(1);
    let one = keyed(0, b"one", &conduit_core::encode_count(1));
    assert_eq!(step_input(&mut operation, 0, &one), StepOutcome::Progress);
    commit(&mut operation);
    let two = keyed(0, b"two", &conduit_core::encode_count(2));
    assert_eq!(step_input(&mut operation, 0, &two), StepOutcome::Await);
    assert_eq!(operation.pending[0].count, 1);
}

#[test]
fn early_close_discards_impossible_side_but_retains_matchable_side() {
    let mut operation = operation(2);
    let left = keyed(0, b"keep", &conduit_core::encode_count(4));
    assert_eq!(step_input(&mut operation, 0, &left), StepOutcome::Progress);
    commit(&mut operation);
    let right = keyed(1, b"impossible", &[0]);
    assert_eq!(step_input(&mut operation, 1, &right), StepOutcome::Progress);
    commit(&mut operation);

    let mut io = StepIo::test_frame(
        [None, None],
        [true, false],
        [Some(operation.output.maximum_bytes()), None],
        None,
        32,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
        StepOutcome::Progress
    );
    commit(&mut operation);
    assert!(operation.closed[0]);
    assert_eq!(operation.pending[0].count, 1);
    assert_eq!(operation.pending[1].count, 0);

    let right = keyed(1, b"keep", &[1]);
    assert_eq!(step_input(&mut operation, 1, &right), StepOutcome::Progress);
    assert_eq!(staged_fields(&operation).0, b"keep");
    commit(&mut operation);

    let mut io = StepIo::test_frame(
        [None, None],
        [false, true],
        [Some(operation.output.maximum_bytes()), None],
        None,
        32,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
        StepOutcome::Complete
    );
}

#[test]
fn closed_inventory_preserves_both_all_close_profiles() {
    let installed = InstalledBack::FlowJoinByKey(Box::new(operation(2)));
    let contracts = <InstalledBack as StepBack<2>>::terminal_transductions(&installed);
    assert!(contracts.into_iter().enumerate().all(|(side, contract)| {
        contract.is_some_and(|contract| {
            contract.input == PortId(side as u16)
                && contract.normal_close == AssignedNormalCloseTransduction::PropagateWhenAllClose
        })
    }));
}
