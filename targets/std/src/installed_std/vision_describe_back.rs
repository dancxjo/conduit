//! Installed four-input Vision description lifecycle.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::LOCAL_VISION_DESCRIBE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct VisionDescribeBack {
    submitted: u8,
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for VisionDescribeBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(10);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 11,
                }));
            }
            if let Some(output) = outcome.output {
                if self.submitted != 0b1111 {
                    return fail(12);
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume_host_completion()
                    .expect("observed visual description completion");
                io.send(PortId(0), output.value)
                    .expect("ready visual impression output");
                self.pending = None;
                self.emitted = true;
                return StepOutcome::Progress;
            }
            io.consume_host_completion()
                .expect("observed partial visual description admission");
            self.pending = None;
            if self.submitted == 0b1111 {
                return fail(13);
            }
            return StepOutcome::Progress;
        }
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        for index in 0..PORTS.min(4) {
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
                return fail(14);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return fail(15);
            };
            io.consume(port).expect("present visual description input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("visual description Host Call");
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
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_DESCRIBE_KIND)
        .expect("reviewed visual description offer")
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
        || placement.resources.len() != 1
        || placement.resources[0].protected.is_none()
        || placement.authority.len() != 1
        || !placement.configuration.is_empty()
    {
        return Err("planned visual description differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 5,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32).saturating_mul(5),
        host_requests: 4,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::VisionDescribe(VisionDescribeBack {
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

    fn operation() -> VisionDescribeBack {
        VisionDescribeBack {
            submitted: 0,
            pending: None,
            next_request: 0,
            emitted: false,
        }
    }

    #[test]
    fn four_exact_inputs_complete_one_impression_without_early_output() {
        let mut operation = operation();
        for index in 0..4usize {
            let mut inputs = [None; 4];
            inputs[index] = Some(value(index as u16 + 1));
            let mut io =
                StepIo::test_frame(inputs, [false; 4], [Some(64), None, None, None], None, 8);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None; 4], None)),
                StepOutcome::Progress
            );
            assert_eq!(
                io.test_host_request().map(|request| request.0),
                Some(RequestId(index as u32))
            );

            let final_input = index == 3;
            let output = final_input.then(|| {
                BoundedValueRef::new(value(9), MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32).unwrap()
            });
            let mut io = StepIo::test_frame(
                [None; 4],
                [false; 4],
                [Some(64), None, None, None],
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
                operation.step(&mut io, &StepInputBytes::test_frame([None; 4], None)),
                StepOutcome::Progress
            );
            assert_eq!(io.test_output(PortId(0)), final_input.then(|| value(9)));
        }
        let mut io =
            StepIo::test_frame([None; 4], [false; 4], [Some(64), None, None, None], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 4], None)),
            StepOutcome::Complete
        );
    }

    #[test]
    fn early_provider_output_and_missing_final_output_fail_closed() {
        let mut early = operation();
        early.submitted = 1;
        early.pending = Some(RequestId(0));
        let output = BoundedValueRef::new(value(9), 64).unwrap();
        let mut io = StepIo::test_frame(
            [None; 4],
            [false; 4],
            [Some(64), None, None, None],
            Some((
                RequestId(0),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )),
            8,
        );
        assert!(matches!(
            early.step(&mut io, &StepInputBytes::test_frame([None; 4], None)),
            StepOutcome::Fail(_)
        ));

        let mut missing = operation();
        missing.submitted = 0b1111;
        missing.pending = Some(RequestId(3));
        let mut io = StepIo::test_frame(
            [None; 4],
            [false; 4],
            [Some(64), None, None, None],
            Some((
                RequestId(3),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )),
            8,
        );
        assert!(matches!(
            missing.step(&mut io, &StepInputBytes::test_frame([None; 4], None)),
            StepOutcome::Fail(_)
        ));
    }
}
