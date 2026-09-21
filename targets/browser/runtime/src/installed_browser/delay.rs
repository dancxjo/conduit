//! Ordered finite Boolean delay through the admitted browser timer boundary.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserBack, BROWSER_TIMER_MAXIMUM_MILLIS};
use conduit_core::{
    encode_monotonic_duration, resource_requirement, ConfigurationValue, PlannedGear,
    TIMER_RESOURCE_CLASS,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

const IMPLEMENTATION: &str = "browser/kernel-time-delay-bool@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-delay@1";

pub(super) static TIME_DELAY: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::time_delay_contract(),
        conduit_semantic_catalog::TIME_DELAY_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![conduit_core::wait_host_call_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    let duration = configured(placement, "duration-ms")?;
    if duration > BROWSER_TIMER_MAXIMUM_MILLIS {
        return Err("time/delay duration-ms exceeds the browser timer bound".into());
    }
    let maximum_values = usize::try_from(configured(placement, "maximum-values")?)
        .map_err(|_| "time/delay maximum-values does not fit")?;
    if maximum_values == 0
        || maximum_values > conduit_semantic_catalog::TIME_MAXIMUM_VALUES as usize
    {
        return Err("time/delay maximum-values exceeds the browser bound".into());
    }
    let durations = (0..maximum_values)
        .map(|_| {
            values
                .store(&encode_monotonic_duration(duration))
                .map_err(debug_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BrowserBack::installed_step(DelayBack {
        durations,
        queued: Vec::with_capacity(maximum_values),
        maximum_values,
        accepted: 0,
        next: 0,
        pending: None,
        closing: false,
        next_request: 0,
    }))
}

fn configured(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("time/delay configuration '{key}' is missing"))
}

struct DelayBack {
    durations: Vec<ValueRef>,
    queued: Vec<ValueRef>,
    maximum_values: usize,
    accepted: usize,
    next: usize,
    pending: Option<RequestId>,
    closing: bool,
    next_request: usize,
}

impl DelayBack {
    fn request<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> Result<(), StepOutcome> {
        let Some(duration) = self.durations.get(self.next_request).copied() else {
            return Err(fail(40));
        };
        let Ok(raw) = u32::try_from(self.next_request + 1) else {
            return Err(fail(40));
        };
        let request = RequestId(raw);
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(duration, 8).expect("duration is exactly eight bytes"),
        )
        .expect("delay timer Host Call");
        self.next_request += 1;
        self.pending = Some(request);
        Ok(())
    }

    fn discard_unused_durations<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        for value in self.durations.drain(self.next_request..) {
            io.discard(value).expect("unused delay duration");
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DelayBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(40);
            }
            if outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return outcome.failure.map_or_else(|| fail(40), StepOutcome::Fail);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(value) = self.queued.get(self.next).copied() else {
                return fail(40);
            };
            io.consume_host_completion()
                .expect("observed delay completion");
            io.send(PortId(0), value).expect("ready delayed output");
            self.pending = None;
            self.next += 1;
            if self.next < self.queued.len() {
                if let Err(outcome) = self.request(io) {
                    return outcome;
                }
            } else if self.closing {
                self.discard_unused_durations(io);
                return StepOutcome::Complete;
            }
            return StepOutcome::Progress;
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.closing || self.accepted >= self.maximum_values {
                return fail(40);
            }
            let retained = io.take_input(PortId(0)).expect("present delay input");
            debug_assert_eq!(retained, value);
            self.queued.push(retained);
            self.accepted += 1;
            if self.pending.is_none() && self.next + 1 == self.queued.len() {
                if let Err(outcome) = self.request(io) {
                    return outcome;
                }
            }
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && !self.closing {
            io.consume_closed(PortId(0))
                .expect("observed delay input closure");
            self.closing = true;
            if self.pending.is_none() {
                if self.next < self.queued.len() {
                    if let Err(outcome) = self.request(io) {
                        return outcome;
                    }
                } else {
                    self.discard_unused_durations(io);
                    return StepOutcome::Complete;
                }
            }
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.queued.clear();
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostCallOutcome, ValueRef};

    fn value(slot: u16, byte_len: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len,
        }
    }

    #[test]
    fn ordered_delay_retains_and_drains_admitted_values() {
        let mut operation = DelayBack {
            durations: vec![value(10, 8), value(11, 8)],
            queued: Vec::with_capacity(2),
            maximum_values: 2,
            accepted: 0,
            next: 0,
            pending: None,
            closing: false,
            next_request: 0,
        };
        let first = value(1, 1);
        let second = value(2, 1);
        let mut first_io = StepIo::test_frame([Some(first)], [false], [Some(1)], None, 4);
        assert_eq!(
            operation.step(&mut first_io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert!(first_io.test_retained(PortId(0)));
        assert_eq!(
            first_io.test_host_request().map(|request| request.0),
            Some(RequestId(1))
        );
        let mut second_io = StepIo::test_frame([Some(second)], [false], [Some(1)], None, 4);
        assert_eq!(
            operation.step(&mut second_io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert!(second_io.test_retained(PortId(0)));
        let mut close_io = StepIo::test_frame([None], [true], [Some(1)], None, 4);
        assert_eq!(
            operation.step(&mut close_io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        for (request, expected) in [(1, first), (2, second)] {
            let completion = HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            };
            let mut io = StepIo::test_frame(
                [None],
                [false],
                [Some(1)],
                Some((RequestId(request), completion)),
                5,
            );
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
                if request == 1 {
                    StepOutcome::Progress
                } else {
                    StepOutcome::Complete
                }
            );
            assert_eq!(io.test_output(PortId(0)), Some(expected));
            if request == 1 {
                assert_eq!(
                    io.test_host_request().map(|request| request.0),
                    Some(RequestId(2))
                );
            }
        }
    }
}
