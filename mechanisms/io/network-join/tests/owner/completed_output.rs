//! The exact retained record is historical data, never another Join authority.
use super::*;
#[test]
fn completed_output_borrows_original_bytes_without_allocating_or_rearming() {
    let state = Rc::new(State::default());
    let (mut owner, node) = ready(state.clone());
    assert!(owner.completed_output().is_none());
    let (bytes, len) = request();
    assert!(run(owner.invoke(node, HostCallId(0), RequestId(1), &bytes[..len])).is_err());
    assert!(owner.completed_output().is_none());
    let published = run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..len])).unwrap();
    let pointer = published.as_ptr();
    let length = published.len();
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..3 {
            let original = owner.completed_output().unwrap();
            assert_eq!(original.as_ptr(), pointer);
            assert_eq!(original.len(), length);
        }
    });
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
    assert_eq!(state.effects.get(), 1);
    assert!(owner.cleanup_required());
    owner.revoke(node, HostCallId(0)).unwrap();
    run(owner.cleanup()).unwrap();
    assert_eq!(owner.completed_output().unwrap().as_ptr(), pointer);
    assert!(!owner.cleanup_required());
    assert!(run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..len])).is_err());
    assert_eq!(state.effects.get(), 1);
    assert_eq!(state.cleanups.get(), 1);
}
