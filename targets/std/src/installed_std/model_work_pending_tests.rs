use super::*;
use crate::hosted_model_work::ModelWorkTerminal;
use conduit_core::{CapabilityOffer, ResourceOffer};
use conduit_kernel::{HostCallId, NodeId, RequestId, ValueRef};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Adapter(Arc<AtomicUsize>, Arc<AtomicUsize>);
impl HostedModelWorkAdapter for Adapter {
    fn capability_offer(&self) -> &CapabilityOffer {
        panic!("not an admission test")
    }
    fn resource_offer(&self) -> &ResourceOffer {
        panic!("not an admission test")
    }
    fn execute(&mut self, _: &PlannedGear, _: &[u8], _: &mut Vec<u8>) -> ModelWorkTerminal {
        panic!("not an execution test")
    }
    fn cancel(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn unload(&mut self) {
        self.1.fetch_add(1, Ordering::SeqCst);
    }
}
fn request() -> HostCallRequest {
    HostCallRequest {
        node: NodeId(2),
        request: RequestId(7),
        call: HostCallId(3),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 0,
            },
            0,
        )
        .unwrap(),
    }
}
fn cancellation(r: HostCallRequest) -> HostCallCancellation {
    HostCallCancellation {
        node: r.node,
        request: r.request,
        call: r.call,
    }
}
#[test]
fn stale_node_call_or_request_cannot_retire_current_worker_and_duplicate_is_idempotent() {
    let cancels = Arc::new(AtomicUsize::new(0));
    let unloads = Arc::new(AtomicUsize::new(0));
    let mut adapter = Adapter(cancels.clone(), unloads.clone());
    let mut host = InstalledModelWorkHost::new(Some(&mut adapter));
    let r = request();
    host.last_request = Some(r);
    host.pending = Some(Pending {
        request: r,
        maximum_output_bytes: 32,
    });
    for foreign in [
        HostCallCancellation {
            node: NodeId(1),
            ..cancellation(r)
        },
        HostCallCancellation {
            call: HostCallId(2),
            ..cancellation(r)
        },
        HostCallCancellation {
            request: RequestId(6),
            ..cancellation(r)
        },
    ] {
        host.cancel(foreign);
        assert!(host.is_pending());
        assert_eq!(cancels.load(Ordering::SeqCst), 0);
    }
    host.cancel(cancellation(r));
    assert!(!host.is_pending());
    assert_eq!(cancels.load(Ordering::SeqCst), 1);
    host.cancel(cancellation(r));
    drop(host);
    assert_eq!(cancels.load(Ordering::SeqCst), 1);
    assert_eq!(unloads.load(Ordering::SeqCst), 0);
}
#[test]
fn early_return_retires_pending_worker_but_completed_owner_stays_warm() {
    for pending in [false, true] {
        let cancels = Arc::new(AtomicUsize::new(0));
        let unloads = Arc::new(AtomicUsize::new(0));
        let mut adapter = Adapter(cancels.clone(), unloads.clone());
        let mut host = InstalledModelWorkHost::new(Some(&mut adapter));
        host.last_request = Some(request());
        if pending {
            host.pending = Some(Pending {
                request: request(),
                maximum_output_bytes: 32,
            });
        }
        drop(host);
        assert_eq!(cancels.load(Ordering::SeqCst), usize::from(pending));
        assert_eq!(unloads.load(Ordering::SeqCst), usize::from(pending));
    }
}
