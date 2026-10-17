//! Retained normal terminal evidence is checked after the sealed Play.
pub fn normal(kernel: &conduit_composite::KernelCompositeHost) {
    use conduit_kernel::KernelEventKind;
    let signs = kernel.signs();
    assert!(!signs.is_empty());
    for events in signs.values() {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == KernelEventKind::RemoteOutputClosed)
                .count(),
            1
        );
        assert!(events
            .iter()
            .any(|event| event.kind == KernelEventKind::BackCompleted));
        assert!(!events.iter().any(|event| matches!(
            event.kind,
            KernelEventKind::BackFailed | KernelEventKind::RunCancelled
        )));
    }
}
