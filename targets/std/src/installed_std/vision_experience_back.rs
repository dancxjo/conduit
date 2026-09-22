//! Installed five-input visual Experience lifecycle.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::LOCAL_VISION_EXPERIENCE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct VisionExperienceBack {
    submitted: u8,
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for VisionExperienceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(20);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 21,
                }));
            }
            if let Some(output) = outcome.output {
                if self.submitted != 0b1_1111 {
                    return fail(22);
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume_host_completion()
                    .expect("observed visual Experience completion");
                io.send(PortId(0), output.value)
                    .expect("ready visual Experience output");
                self.pending = None;
                self.emitted = true;
                return StepOutcome::Progress;
            }
            io.consume_host_completion()
                .expect("observed partial visual Experience admission");
            self.pending = None;
            if self.submitted == 0b1_1111 {
                return fail(23);
            }
            return StepOutcome::Progress;
        }
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        for index in 0..PORTS.min(5) {
            let mask = 1u8 << index;
            if self.submitted & mask != 0 {
                continue;
            }
            let port = PortId(index as u16);
            let Some(value) = io.input(port) else {
                continue;
            };
            let Ok(input) = BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
            else {
                return fail(24);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return fail(25);
            };
            io.consume(port).expect("present visual Experience input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("visual Experience Host Call");
            self.submitted |= mask;
            self.pending = Some(request);
            self.next_request = next;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.emitted = true;
    }
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

fn offer() -> conduit_core::CapabilityOffer {
    conduit_std_offers::local_vision_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_EXPERIENCE_KIND)
        .expect("reviewed visual Experience offer")
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.configuration.is_empty()
    {
        return Err("planned visual Experience differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 6,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32).saturating_mul(6),
        host_requests: 5,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::VisionExperience(VisionExperienceBack {
        submitted: 0,
        pending: None,
        next_request: 0,
        emitted: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostCallOutcome, ValueRef};

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: 64,
        }
    }

    #[test]
    fn five_inputs_complete_exactly_once() {
        let mut back = VisionExperienceBack {
            submitted: 0,
            pending: None,
            next_request: 0,
            emitted: false,
        };
        for index in 0..5usize {
            let mut inputs = [None; 5];
            inputs[index] = Some(value(index as u16 + 1));
            let mut io = StepIo::test_frame(inputs, [false; 5], [Some(64); 5], None, 8);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None; 5], None)),
                StepOutcome::Progress
            );
            let output = (index == 4).then(|| BoundedValueRef::new(value(9), 64).unwrap());
            let mut io = StepIo::test_frame(
                [None; 5],
                [false; 5],
                [Some(64); 5],
                Some((
                    RequestId(index as u32),
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output,
                        failure: None,
                    },
                )),
                8,
            );
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None; 5], None)),
                StepOutcome::Progress
            );
        }
    }
}
