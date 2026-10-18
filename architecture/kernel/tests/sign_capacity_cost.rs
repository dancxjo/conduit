//! Native observations of actual Sign-capacity reservations. Preparation and
//! snapshot checks are outside timing; the timed loop includes result stores
//! and black-box barriers. No performance threshold or CPU share is asserted.
#![cfg(feature = "alloc")]
use conduit_kernel::{
    FixedSignLog, HostedSignLog, KernelEvent, KernelEventKind, NodeId, SignError, SignQuery,
    SignSink,
};
use std::{hint::black_box, mem::size_of, time::Instant};

const CALLS: usize = 256;
#[derive(Clone, Copy, Debug)]
enum State {
    Empty,
    Sparse,
    HalfTransient,
    FullTransient,
    FullSticky,
}
const STATES: [State; 5] = [
    State::Empty,
    State::Sparse,
    State::HalfTransient,
    State::FullTransient,
    State::FullSticky,
];

fn seed(log: &mut impl SignSink, state: State) {
    let capacity = usize::from(log.item_capacity());
    let (count, kind) = match state {
        State::Empty => (0, KernelEventKind::ValueConsumed),
        State::Sparse => (1, KernelEventKind::BackCompleted),
        State::HalfTransient => (capacity / 2, KernelEventKind::ValueConsumed),
        State::FullTransient => (capacity, KernelEventKind::ValueConsumed),
        State::FullSticky => (capacity, KernelEventKind::BackCompleted),
    };
    for _ in 0..count {
        log.record(NodeId(0), None, None, kind).unwrap();
    }
}

fn observe<L: SignSink + SignQuery>(
    log: &L,
    events: impl Iterator<Item = KernelEvent>,
    kind: &str,
    state: State,
    trial: usize,
    print: bool,
) {
    let capacity = log.item_capacity();
    let before = (log.len(), log.used_bytes(), log.retention_gap());
    let expected = match state {
        State::FullSticky => Err(SignError::ItemCapacityExceeded),
        State::Sparse if capacity == 1 => Err(SignError::ItemCapacityExceeded),
        _ => Ok(()),
    };
    // Consume the supplied observation before timing so it cannot be confused
    // with the sink's own reservation traversal.
    let retained = events.count();
    assert_eq!(retained, usize::from(log.len()));
    let mut results = [Ok(()); CALLS];
    let started = Instant::now();
    for result in &mut results {
        *result = black_box(black_box(log).ensure_capacity(black_box(1)));
    }
    let elapsed_ns = started.elapsed().as_nanos();
    assert!(results.into_iter().all(|result| result == expected));
    assert_eq!(before, (log.len(), log.used_bytes(), log.retention_gap()));
    if print {
        println!(
            "SIGN_CAPACITY_COST_ROW {{\"sink\":\"{kind}\",\"capacity\":{capacity},\"state\":\"{state:?}\",\"trial\":{trial},\"calls\":{CALLS},\"elapsed_ns\":{elapsed_ns},\"occupied\":{},\"used_bytes\":{},\"accepted\":{}}}",
            log.len(), log.used_bytes(), expected.is_ok()
        );
    }
}

fn fixed<const N: usize>(trial: usize, print: bool) {
    for state in STATES {
        let mut log = FixedSignLog::<N>::new((N * size_of::<KernelEvent>()) as u32).unwrap();
        seed(&mut log, state);
        let events = log.events().collect::<Vec<_>>();
        observe(&log, events.iter().copied(), "fixed", state, trial, print);
        assert!(events.into_iter().eq(log.events()));
    }
}
fn hosted(capacity: u16, trial: usize, print: bool) {
    for state in STATES {
        let mut log = HostedSignLog::new(
            capacity,
            u32::from(capacity) * size_of::<KernelEvent>() as u32,
        )
        .unwrap();
        seed(&mut log, state);
        let events = log.events().collect::<Vec<_>>();
        observe(&log, events.iter().copied(), "hosted", state, trial, print);
        assert!(events.into_iter().eq(log.events()));
    }
}

#[test]
fn capacity_probe_preserves_empty_sparse_full_and_sticky_logs() {
    fixed::<1>(0, false);
    fixed::<8>(0, false);
    hosted(1, 0, false);
    hosted(8, 0, false);
}

#[test]
#[ignore = "manual Sign reservation cost capture; no timing threshold"]
fn observe_production_sign_capacity_cost() {
    for trial in 0..8 {
        fixed::<1>(trial, true);
        fixed::<8>(trial, true);
        fixed::<128>(trial, true);
        fixed::<4096>(trial, true);
        for capacity in [1, 8, 128, 4096, 60000] {
            hosted(capacity, trial, true);
        }
    }
}
