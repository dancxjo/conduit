//! Test-only finite pulse sink. This is byte-checking instrumentation, not manifestation.
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{CapabilityId, CapabilityOffer, PlannedGear, PortDirection};
use conduit_kernel::{HostedValueStore, OperationAction, OperationInput, PortId};
pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: "conduit-test/pulse-sink@1",
    budget: |_| {
        Ok(OperationBudget {
            value_items: 1,
            value_bytes: 1,
            host_requests: 0,
            sign_items: 64,
            maximum_value_bytes: 6,
        })
    },
    prepare,
};
pub(super) fn offer() -> CapabilityOffer {
    let mut offer = conduit_std_offers::pulse_observe_offer();
    offer.kind_id = "conduit-test/pulse-sink".into();
    offer.kind_contract_revision = "conduit-test/pulse-sink@1".into();
    offer.capability_id = CapabilityId::from("conduit-test-pulse-sink");
    offer.implementation.implementation_id = FACTORY.implementation_id.into();
    offer.startup_parameters.clear();
    offer.inputs = core::mem::take(&mut offer.outputs);
    offer.inputs[0].direction = PortDirection::Input;
    offer
}
fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let offer = offer();
    if placement.kind_id != offer.kind_id
        || placement.inputs != offer.inputs
        || !placement.outputs.is_empty()
    {
        return Err("invalid test pulse sink".into());
    }
    Ok(InstalledOperation::TestPulseSink(Sink { next: 0 }))
}
pub(super) struct Sink {
    next: u32,
}
impl Sink {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        assert_eq!(input, OperationInput::Closed { port: PortId(0) });
        assert_eq!(self.next, 3);
        OperationAction::Complete
    }
    pub(super) fn resume_value(&mut self, port: PortId, canonical: &[u8]) -> OperationAction {
        assert_eq!(port, PortId(0));
        let pulse = conduit_time::decode_pulse_observation(canonical).unwrap();
        assert_eq!((pulse.sequence, pulse.period_ms), (self.next, 320));
        self.next += 1;
        OperationAction::Await
    }
}
