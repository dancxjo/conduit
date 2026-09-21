//! Existing browser pointer offer installed in the ordinary form runner.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) const HOST_CALL: &str = "browser.host/pointer-source@1";
pub(super) static POINTER: BrowserInstallation = BrowserInstallation {
    implementation_id: "browser/form-pointer-source@1",
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    crate::browser_pointer::pointer_source_offer(
        "browser-form-pointer-source@1",
        "browser/form-pointer-source@1",
        "browser/form-pointer-source@1",
        "conduit-browser-runtime/form-pointer-source@1",
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        vec![conduit_core::ResourceRequirement {
            class_id: super::input::WINDOW_INPUT_RESOURCE_CLASS.into(),
            units: 1,
            content: None,
            protected_role: None,
            compute: None,
        }],
    )
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    let empty = values
        .store(&[])
        .map_err(|error| format!("pointer request: {error:?}"))?;
    Ok(BrowserBack::installed_step(PointerSource {
        empty,
        pending: false,
        next: 0,
    }))
}

struct PointerSource {
    empty: ValueRef,
    pending: bool,
    next: u32,
}

impl PointerSource {
    fn request<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        self.pending = true;
        io.request_host_call(
            RequestId(self.next),
            HostCallId(0),
            BoundedValueRef::new(self.empty, 0).expect("empty pointer request"),
        )
        .expect("pointer Host Call");
    }
}

impl<const PORTS: usize> StepBack<PORTS> for PointerSource {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(self.next) {
                return fail();
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None)
                    if output.admitted_bytes == super::MAXIMUM_BROWSER_VALUE_BYTES as u32 =>
                {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(next) = self.next.checked_add(1) else {
                        return identity_exhausted();
                    };
                    io.consume_host_completion()
                        .expect("observed pointer completion");
                    io.send(PortId(0), output.value)
                        .expect("ready pointer output");
                    self.pending = false;
                    self.next = next;
                    self.request(io);
                    StepOutcome::Progress
                }
                (HostCallDisposition::Failed, None, Some(failure)) => StepOutcome::Fail(failure),
                _ => fail(),
            }
        } else if self.pending {
            StepOutcome::Await
        } else {
            self.request(io);
            StepOutcome::Progress
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}

fn fail() -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidInput,
        detail: 21,
    })
}

fn identity_exhausted() -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::IdentityCapacityExhausted,
        detail: 21,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostCallOutcome;

    #[test]
    fn pointer_rearms_after_multiple_separated_observations() {
        let mut operation = PointerSource {
            empty: ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 0,
            },
            pending: false,
            next: 0,
        };
        let mut io = StepIo::test_frame(
            [None; super::super::BROWSER_PORTS_PER_GEAR],
            [false; super::super::BROWSER_PORTS_PER_GEAR],
            [Some(super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32);
                super::super::BROWSER_PORTS_PER_GEAR],
            None,
            4,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None; super::super::BROWSER_PORTS_PER_GEAR], None,),
            ),
            StepOutcome::Progress
        );
        for slot in 1..=3 {
            assert_eq!(
                io.test_host_request().map(|request| request.0),
                Some(RequestId((slot - 1).into()))
            );
            let value = ValueRef {
                slot,
                generation: 1,
                byte_len: 16,
            };
            io = StepIo::test_frame(
                [None; super::super::BROWSER_PORTS_PER_GEAR],
                [false; super::super::BROWSER_PORTS_PER_GEAR],
                [Some(super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32);
                    super::super::BROWSER_PORTS_PER_GEAR],
                Some((
                    RequestId((slot - 1).into()),
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
                            )
                            .unwrap(),
                        ),
                        failure: None,
                    },
                )),
                4,
            );
            assert_eq!(
                operation.step(
                    &mut io,
                    &StepInputBytes::test_frame(
                        [None; super::super::BROWSER_PORTS_PER_GEAR],
                        None,
                    ),
                ),
                StepOutcome::Progress
            );
            assert_eq!(io.test_output(PortId(0)), Some(value));
        }
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(3))
        );
    }
}
