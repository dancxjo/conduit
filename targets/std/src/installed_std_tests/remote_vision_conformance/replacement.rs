//! Replacement-owner controls; local descriptors are descriptive, not grants.
use crate::AdmittedRemoteFragment;
use conduit_kernel::{
    scheduler::{HostCallRequest, SchedulerStatus},
    KernelEventKind,
};

pub(super) fn pending(
    replacement: &mut AdmittedRemoteFragment,
    sibling: &mut AdmittedRemoteFragment,
    old: HostCallRequest,
    encoded: &[u8],
) -> HostCallRequest {
    let request = (0..32)
        .find_map(|_| {
            if let Some(request) = replacement.next_host_request() {
                return Some(request);
            }
            replacement.step().unwrap();
            None
        })
        .expect("B reaches its own genuinely dispatched Vision request");
    // Reuse of these exact receiver-local fields must not be confused with
    // retention of A's consumed owner or permission to transplant into B.
    assert_eq!(
        request, old,
        "the control must repeat the complete local descriptor"
    );
    assert_eq!(
        replacement.host_request_view(request).unwrap().input,
        encoded
    );
    assert!((0..32).any(|_| replacement.step().unwrap() == SchedulerStatus::Idle));
    let identity = replacement.identity().clone();
    let sibling_identity = sibling.identity().clone();
    let signs: Vec<_> = replacement.kernel_signs().events().collect();
    let sibling_signs: Vec<_> = sibling.kernel_signs().events().collect();
    assert_eq!(completion_count(replacement, request), 0);
    assert!(!signs
        .iter()
        .any(|event| event.node == request.node && event.kind == KernelEventKind::ValueRouted));
    // No async callback ingress exists on this synchronous owner. These actual
    // pending steps establish absence of spontaneous completion/output; they
    // are not a fabricated late-A callback or cross-owner refusal attempt.
    for _ in 0..3 {
        assert!(replacement.next_host_request().is_none());
        assert_eq!(replacement.step().unwrap(), SchedulerStatus::Idle);
    }
    assert_eq!(replacement.identity(), &identity);
    assert_eq!(
        replacement.host_request_view(request).unwrap().input,
        encoded
    );
    assert_eq!(
        replacement.kernel_signs().events().collect::<Vec<_>>(),
        signs
    );
    assert_eq!(sibling.identity(), &sibling_identity);
    assert_eq!(
        sibling.kernel_signs().events().collect::<Vec<_>>(),
        sibling_signs
    );
    request
}

pub(super) fn completion_count(
    runtime: &AdmittedRemoteFragment,
    request: HostCallRequest,
) -> usize {
    runtime
        .kernel_signs()
        .events()
        .filter(|event| {
            event.node == request.node
                && event.request == Some(request.request)
                && event.kind == KernelEventKind::HostCallCompleted
        })
        .count()
}
