//! Test-only finite pulse sink. This is byte-checking instrumentation, not manifestation.
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{CapabilityId, CapabilityOffer, PlannedGear, PortDirection};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    HostedValueStore, OperationAction, OperationInput, PortId,
};
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
impl<const PORTS: usize> StepOperation<PORTS> for Sink {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let pulse = conduit_time::decode_pulse_observation(
                input_bytes.input(PortId(0)).expect("present pulse bytes"),
            )
            .expect("canonical pulse fixture");
            assert_eq!((pulse.sequence, pulse.period_ms), (self.next, 320));
            io.consume(PortId(0)).expect("present pulse fixture");
            self.next += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            assert_eq!(self.next, 3);
            io.consume_closed(PortId(0))
                .expect("observed pulse fixture closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
impl Sink {}
