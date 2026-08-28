use conduit_kernel::static_merge::{
    FixedStaticMerge, StaticMergeError, StaticMergeEvent, StaticMergeSource,
};
use conduit_kernel::{NodeId, PortId, ValueRef};

fn source(node: u16) -> StaticMergeSource {
    StaticMergeSource {
        node: NodeId(node),
        port: PortId(0),
    }
}

fn event(sequence: u64, source: StaticMergeSource) -> StaticMergeEvent {
    StaticMergeEvent {
        sequence,
        source,
        value: ValueRef {
            slot: sequence as u16,
            generation: 1,
            byte_len: 9,
        },
    }
}

#[test]
fn exact_static_sources_merge_in_kernel_order() {
    let terminal = source(0);
    let browser_a = source(1);
    let browser_b = source(2);
    let mut merge = FixedStaticMerge::<3, 6>::new([terminal, browser_a, browser_b]).unwrap();
    merge.offer(event(0, terminal)).unwrap();
    merge.offer(event(1, browser_b)).unwrap();
    merge.offer(event(2, browser_a)).unwrap();
    assert_eq!(merge.pop().unwrap().source, terminal);
    assert_eq!(merge.pop().unwrap().source, browser_b);
    assert_eq!(merge.pop().unwrap().source, browser_a);
    assert!(merge.is_empty());
}

#[test]
fn unknown_source_reordering_and_pressure_fail_distinctly() {
    let terminal = source(0);
    let mut merge = FixedStaticMerge::<1, 1>::new([terminal]).unwrap();
    assert_eq!(
        merge.offer(event(0, source(7))),
        Err(StaticMergeError::UnknownSource)
    );
    merge.offer(event(1, terminal)).unwrap();
    assert_eq!(
        merge.offer(event(2, terminal)),
        Err(StaticMergeError::QueueFull)
    );
    merge.pop().unwrap();
    assert_eq!(
        merge.offer(event(1, terminal)),
        Err(StaticMergeError::DuplicateOrOutOfOrderSequence)
    );
}
