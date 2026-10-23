//! Project a typed tick occurrence into an Empty trigger without inventing time.
use alloc::vec;
use conduit_core::*;

pub const TICK_TRIGGER_KIND: &str = "time/tick-trigger";
pub fn tick_trigger_semantic_contract() -> Kind {
    let port = |name, kind, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(TICK_TRIGGER_KIND),
        kind_contract_revision: "conduit.time/tick-trigger@1".into(),
        startup_parameters: vec![],
        shorthand: Some((port_id("tick"), port_id("begin"))),
        inputs: vec![port("tick", crate::TICK_VALUE_KIND, PortDirection::Input)],
        outputs: vec![port("begin", EMPTY_INFO_ID, PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}
#[cfg(feature = "kernel-step")]
mod back {
    use conduit_kernel::{
        scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
        CanonicalValue, Failure, FailureCode, PortId,
    };
    #[derive(Default)]
    pub struct TickTriggerBack {
        cancelled: bool,
    }
    impl<const P: usize> StepBack<P> for TickTriggerBack {
        fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
            let fail = |code, detail| StepOutcome::Fail(Failure { code, detail });
            if self.cancelled {
                return fail(FailureCode::Cancelled, 1);
            }
            if io.input_abnormal(PortId(0)).is_some() {
                return fail(FailureCode::InvalidInput, 1);
            }
            if let Some(value) = io.input(PortId(0)) {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if value.byte_len != crate::TICK_ENCODED_LEN
                    || input
                        .input(PortId(0))
                        .is_none_or(|bytes| crate::decode_tick(bytes).is_err())
                {
                    return fail(FailureCode::InvalidInput, 2);
                }
                if io
                    .send_canonical(PortId(0), CanonicalValue::new(&[]).unwrap())
                    .is_err()
                    || io.consume(PortId(0)).is_err()
                {
                    return fail(FailureCode::InvalidLifecycle, 1);
                }
                StepOutcome::Progress
            } else if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    return fail(FailureCode::InvalidLifecycle, 2);
                }
                StepOutcome::Complete
            } else {
                StepOutcome::Await
            }
        }
        fn cancel(&mut self) {
            self.cancelled = true;
        }
    }
}
#[cfg(feature = "kernel-step")]
pub use back::TickTriggerBack;
