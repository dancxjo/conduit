//! Trailing finite Boolean debounce through the admitted browser timer boundary.

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

const IMPLEMENTATION: &str = "browser/kernel-time-debounce-bool@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-debounce@1";

pub(super) static TIME_DEBOUNCE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::time_debounce_contract(),
        conduit_semantic_catalog::TIME_DEBOUNCE_CONTRACT_REVISION,
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
        return Err("time/debounce duration-ms exceeds the browser timer bound".into());
    }
    if !placement.configuration.iter().any(|entry| matches!((&*entry.key, &entry.value), ("policy", ConfigurationValue::Text(value)) if value == conduit_semantic_catalog::TIME_POLICY_TRAILING)) {
        return Err("time/debounce supports only exact trailing policy".into());
    }
    let maximum_values = usize::try_from(configured(placement, "maximum-values")?)
        .map_err(|_| "time/debounce maximum-values does not fit")?;
    if maximum_values == 0
        || maximum_values > conduit_semantic_catalog::TIME_MAXIMUM_VALUES as usize
    {
        return Err("time/debounce maximum-values exceeds the browser bound".into());
    }
    let durations = (0..maximum_values)
        .map(|_| {
            values
                .store(&encode_monotonic_duration(duration))
                .map_err(debug_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BrowserBack::installed_step(DebounceBack {
        durations,
        next_request: 0,
        maximum_values,
        accepted: 0,
        pending: None,
        candidate: None,
        closing: false,
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
        .ok_or_else(|| format!("time/debounce configuration '{key}' is missing"))
}

struct DebounceBack {
    durations: Vec<ValueRef>,
    next_request: usize,
    maximum_values: usize,
    accepted: usize,
    pending: Option<RequestId>,
    candidate: Option<ValueRef>,
    closing: bool,
}

impl DebounceBack {
    fn arm<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> Result<(), StepOutcome> {
        let Some(duration) = self.durations.get(self.next_request).copied() else {
            return Err(fail(51));
        };
        let request = RequestId(u32::try_from(self.next_request + 1).map_err(|_| fail(51))?);
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(duration, 8).expect("duration is eight bytes"),
        )
        .expect("debounce timer Host Call");
        self.next_request += 1;
        self.pending = Some(request);
        Ok(())
    }
    fn discard_unused<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        for value in self.durations.drain(self.next_request..) {
            io.discard(value).expect("unused debounce duration");
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DebounceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        // A simultaneously visible value replaces the candidate before the old deadline can emit.
        if let Some(value) = io.input(PortId(0)) {
            if self.closing || self.accepted >= self.maximum_values {
                return fail(52);
            }
            let retained = io.take_input(PortId(0)).expect("present debounce input");
            debug_assert_eq!(retained, value);
            if let Some(previous) = self.candidate.replace(retained) {
                io.discard(previous).expect("superseded candidate");
            }
            self.accepted += 1;
            if let Some((request, outcome)) = io.host_completion() {
                if self.pending != Some(request)
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                    || !matches!(
                        outcome.disposition,
                        HostCallDisposition::Completed | HostCallDisposition::Cancelled
                    )
                {
                    return fail(53);
                }
                io.consume_host_completion()
                    .expect("observed superseded deadline");
                self.pending = None;
                if let Err(outcome) = self.arm(io) {
                    return outcome;
                }
            } else if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("debounce deadline cancellation");
            } else if let Err(outcome) = self.arm(io) {
                return outcome;
            }
            return StepOutcome::Progress;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return outcome.failure.map_or_else(|| fail(53), StepOutcome::Fail);
            }
            match outcome.disposition {
                HostCallDisposition::Cancelled => {
                    io.consume_host_completion()
                        .expect("observed deadline cancellation");
                    self.pending = None;
                    if self.closing {
                        if let Some(value) = self.candidate.take() {
                            if !io.output_ready(PortId(0)) {
                                self.candidate = Some(value);
                                return StepOutcome::Await;
                            }
                            io.send(PortId(0), value).expect("final debounce output");
                        }
                        self.discard_unused(io);
                        return StepOutcome::Complete;
                    }
                    if let Err(outcome) = self.arm(io) {
                        return outcome;
                    }
                    StepOutcome::Progress
                }
                HostCallDisposition::Completed => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(value) = self.candidate.take() else {
                        return fail(54);
                    };
                    io.consume_host_completion()
                        .expect("observed deadline completion");
                    io.send(PortId(0), value).expect("debounced output");
                    self.pending = None;
                    if self.closing {
                        self.discard_unused(io);
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Progress
                    }
                }
                _ => fail(53),
            }
        } else if io.input_closed(PortId(0)) && !self.closing {
            io.consume_closed(PortId(0))
                .expect("observed debounce closure");
            self.closing = true;
            if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("closing debounce deadline cancellation");
                StepOutcome::Progress
            } else {
                if let Some(value) = self.candidate.take() {
                    if !io.output_ready(PortId(0)) {
                        self.candidate = Some(value);
                        return StepOutcome::Await;
                    }
                    io.send(PortId(0), value).expect("final debounce output");
                }
                self.discard_unused(io);
                StepOutcome::Complete
            }
        } else {
            StepOutcome::Await
        }
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }
    fn cancel(&mut self) {
        self.pending = None;
        self.candidate = None;
    }
}

const fn fail(detail: u16) -> StepOutcome {
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
    fn completion(request: u32, disposition: HostCallDisposition) -> (RequestId, HostCallOutcome) {
        (
            RequestId(request),
            HostCallOutcome {
                disposition,
                output: None,
                failure: None,
            },
        )
    }
    fn frame(
        input: Option<ValueRef>,
        closed: bool,
        completion: Option<(RequestId, HostCallOutcome)>,
    ) -> StepIo<1> {
        StepIo::test_frame([input], [closed], [Some(1)], completion, 12)
    }
    fn operation() -> DebounceBack {
        DebounceBack {
            durations: vec![value(10, 8), value(11, 8)],
            next_request: 0,
            maximum_values: 2,
            accepted: 0,
            pending: None,
            candidate: None,
            closing: false,
        }
    }

    #[test]
    fn trailing_replacement_rearms_and_close_flushes_exact_last_value() {
        let mut op = operation();
        let first = value(1, 1);
        let last = value(2, 1);
        let mut io = frame(Some(first), false, None);
        assert_eq!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        let mut io = frame(Some(last), false, None);
        assert_eq!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(io.test_host_cancellation(), Some(RequestId(1)));
        assert!(io.test_discards().contains(&Some(first)));
        let mut io = frame(
            None,
            false,
            Some(completion(1, HostCallDisposition::Cancelled)),
        );
        assert_eq!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(io.test_host_request().map(|r| r.0), Some(RequestId(2)));
        let mut io = frame(None, true, None);
        assert_eq!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        let mut io = frame(
            None,
            false,
            Some(completion(2, HostCallDisposition::Cancelled)),
        );
        assert_eq!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert_eq!(io.test_output(PortId(0)), Some(last));
    }

    #[test]
    fn simultaneous_value_wins_over_old_completion_and_malformed_completion_refuses() {
        let mut op = operation();
        let mut io = frame(Some(value(1, 1)), false, None);
        let _ = op.step(&mut io, &StepInputBytes::test_frame([None], None));
        let mut io = frame(
            Some(value(2, 1)),
            false,
            Some(completion(1, HostCallDisposition::Completed)),
        );
        assert_eq!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(io.test_output(PortId(0)), None);
        assert_eq!(io.test_host_request().map(|r| r.0), Some(RequestId(2)));
        let mut io = frame(
            None,
            false,
            Some(completion(9, HostCallDisposition::Completed)),
        );
        assert!(matches!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Fail(_)
        ));
    }

    #[test]
    fn finite_pressure_refuses_excess_input_and_cancel_releases_retained_state() {
        let mut op = operation();
        op.maximum_values = 1;
        let mut io = frame(Some(value(1, 1)), false, None);
        assert_eq!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );

        let mut io = frame(Some(value(2, 1)), false, None);
        assert!(matches!(
            op.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Fail(_)
        ));

        <DebounceBack as StepBack<1>>::cancel(&mut op);
        assert_eq!(op.pending, None);
        assert_eq!(op.candidate, None);
    }
}
