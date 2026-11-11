//! Revocation is linearized by the existing kernel, before provider publication.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
struct StopPending {
    inner: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    control: RunControl,
    request: Option<conduit_std_host::RunControlRequestId>,
    cancelled: Arc<AtomicUsize>,
}
impl conduit_std_host::hosted_local_model::HostedLocalModelAdapter for StopPending {
    fn offer(&self) -> &conduit_ai::LocalModelOffer {
        self.inner.offer()
    }
    fn execute(
        &mut self,
        _: &conduit_core::PlannedGear,
        _: &[u8],
        _: &mut Vec<u8>,
    ) -> conduit_std_host::hosted_local_model::LocalModelAdapterTerminal {
        panic!("pending fixture must use begin")
    }
    fn begin_generation(
        &mut self,
        _: &conduit_core::PlannedGear,
        _: &[u8],
        _: &mut Vec<u8>,
    ) -> Option<conduit_std_host::hosted_local_model::LocalModelAdapterTerminal> {
        self.control
            .request_stop(self.request.take().unwrap())
            .unwrap();
        None
    }
    fn poll_generation(
        &mut self,
        _: &mut Vec<u8>,
    ) -> Option<conduit_std_host::hosted_local_model::LocalModelAdapterTerminal> {
        panic!("cancelled generation must lose publication custody before poll")
    }
    fn cancel_generation(&mut self) {
        self.cancelled.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn pete_cadence_cancellation_revokes_pending_model_publication() {
    let control = RunControl::default();
    let cancelled = Arc::new(AtomicUsize::new(0));
    let model = StopPending {
        inner: common::fenced_model().0,
        control: control.clone(),
        request: Some(
            conduit_std_host::RunControlRequestId::new("cancel-pending-generation").unwrap(),
        ),
        cancelled: cancelled.clone(),
    };
    let mut harness = harness_with_model(Box::new(model));
    let mut incoming = common::inputs(&[]);
    incoming.extend([frame(1, "The hallway label is COPPER KITE."), trigger(1)]);
    let mut collector = common::Collector::default();
    let report = common::run(&mut harness, &incoming, &mut collector, &control).unwrap();
    assert!(matches!(
        common::terminal(&report),
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    assert!(!collector
        .0
        .iter()
        .any(|item| item.front_port_id.as_str() == "episodes"));
}
