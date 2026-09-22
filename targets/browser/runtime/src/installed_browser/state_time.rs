//! Browser installations for recurring timer-driven current count.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserBack;
use conduit_core::{
    kind_id, resource_requirement, wait_host_call_requirement, ConfigurationValue, PlannedGear,
    PRESENTATION_RESOURCE_CLASS, TIMER_RESOURCE_CLASS,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, CanonicalValue, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
    ValueStorage,
};

const ARTIFACT: &str = "conduit-browser-runtime/installed-state-time@1";
const TIME_EVERY_IMPLEMENTATION: &str = "browser/kernel-time-every@1";
const STATE_COUNT_IMPLEMENTATION: &str = "browser/kernel-state-count@1";
const COUNT_PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-count@1";
const COUNT_PRESENTATION_OPERATION: &str = "conduit.host/browser-present-count@1";
pub(crate) const BROWSER_TIMER_MAXIMUM_MILLIS: u64 = 10_000;

pub(super) static TIME_EVERY: BrowserInstallation = BrowserInstallation {
    implementation_id: TIME_EVERY_IMPLEMENTATION,
    offer: time_every_offer,
    prepare: prepare_time_every,
    perform: None,
};

pub(super) static STATE_COUNT: BrowserInstallation = BrowserInstallation {
    implementation_id: STATE_COUNT_IMPLEMENTATION,
    offer: state_count_offer,
    prepare: prepare_state_count,
    perform: None,
};

pub(super) static COUNT_PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: COUNT_PRESENTATION_IMPLEMENTATION,
    offer: count_presentation_offer,
    prepare: prepare_count_presentation,
    perform: Some(perform_count_presentation),
};

fn time_every_offer() -> conduit_core::CapabilityOffer {
    let mut offer = conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::time_every_contract(),
        conduit_time::TIME_EVERY_CONTRACT_REVISION,
        identity(TIME_EVERY_IMPLEMENTATION),
        vec![wait_host_call_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        Vec::new(),
    );
    offer.startup_parameters[0].has_default = false;
    offer
}

fn state_count_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::state_count_contract(),
        conduit_semantic_catalog::STATE_COUNT_CONTRACT_REVISION,
        identity(STATE_COUNT_IMPLEMENTATION),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn count_presentation_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::count_presentation_contract(),
        conduit_semantic_catalog::COUNT_PRESENTATION_CONTRACT_REVISION,
        identity(COUNT_PRESENTATION_IMPLEMENTATION),
        vec![conduit_core::HostCallRequirement {
            contract_id: conduit_core::HostCallContractId::from(COUNT_PRESENTATION_OPERATION),
            target_kind: Some(kind_id("presentation/browser-count")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_semantic_catalog::COUNT_ENCODED_LEN,
            maximum_output_bytes: 0,
        }],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn identity(
    implementation: &'static str,
) -> conduit_semantic_catalog::RealizationOfferIdentity<'static> {
    conduit_semantic_catalog::RealizationOfferIdentity {
        capability: implementation,
        execution_profile: implementation,
        implementation,
        artifact: ARTIFACT,
    }
}

fn prepare_time_every(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &time_every_offer())?;
    let period_millis =
        quantity_millis_configuration(placement, "freq", BROWSER_TIMER_MAXIMUM_MILLIS)?;
    let wait = values
        .store(&period_millis.to_le_bytes())
        .map_err(debug_error)?;
    Ok(BrowserBack::installed_step(TimeEveryBack {
        wait,
        sequence: 0,
        next_request: 0,
        pending: None,
    }))
}

fn prepare_state_count(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &state_count_offer())?;
    let start = u64_configuration(placement, "start", u64::MAX)?;
    Ok(BrowserBack::installed_step(StateCountBack {
        current: start,
        initial_emitted: false,
    }))
}

fn prepare_count_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &count_presentation_offer())?;
    Ok(BrowserBack::presentation(
        conduit_semantic_catalog::COUNT_ENCODED_LEN,
        1,
    ))
}

fn perform_count_presentation(
    _placement: &PlannedGear,
    input: &[u8],
) -> Result<BrowserHostResult, String> {
    let encoded: [u8; conduit_semantic_catalog::COUNT_ENCODED_LEN as usize] = input
        .try_into()
        .map_err(|_| "count manifestation is not an exact Count")?;
    let _count = u64::from_le_bytes(encoded);
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::COUNT_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn quantity_millis_configuration(
    placement: &PlannedGear,
    key: &str,
    maximum: u64,
) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (found, ConfigurationValue::Quantity(value)) if found == key => value
                .convert(conduit_core::QuantityUnit::Millisecond)
                .ok()
                .and_then(|value| u64::try_from(value.value()).ok())
                .filter(|value| *value <= maximum),
            _ => None,
        })
        .ok_or_else(|| {
            format!(
                "{} configuration '{key}' is missing or exceeds the browser bound",
                placement.kind_id.as_str()
            )
        })
}

fn u64_configuration(placement: &PlannedGear, key: &str, maximum: u64) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key && *value <= maximum => {
                Some(*value)
            }
            _ => None,
        })
        .ok_or_else(|| {
            format!(
                "{} configuration '{key}' is missing or exceeds the browser bound",
                placement.kind_id.as_str()
            )
        })
}

struct TimeEveryBack {
    wait: ValueRef,
    sequence: u64,
    next_request: u32,
    pending: Option<RequestId>,
}

impl TimeEveryBack {
    fn request_wait<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> Result<(), StepOutcome> {
        let request = RequestId(self.next_request);
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(self.wait, conduit_time::TICK_ENCODED_LEN)
                .expect("browser timer duration is exactly eight bytes"),
        )
        .expect("recurring browser timer Host Call");
        self.pending = Some(request);
        Ok(())
    }
}

impl<const PORTS: usize> StepBack<PORTS> for TimeEveryBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return outcome.failure.map_or_else(|| fail(20), StepOutcome::Fail);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(sequence) = self.sequence.checked_add(1) else {
                return capacity_fail(20);
            };
            let Some(next_request) = self.next_request.checked_add(1) else {
                return capacity_fail(20);
            };
            let value = CanonicalValue::new(&conduit_time::encode_tick(self.sequence))
                .expect("Tick has a fixed canonical encoding");
            io.consume_host_completion()
                .expect("observed recurring timer completion");
            io.send_canonical(PortId(0), value)
                .expect("ready recurring Tick output");
            self.pending = None;
            self.sequence = sequence;
            self.next_request = next_request;
            if let Err(outcome) = self.request_wait(io) {
                return outcome;
            }
            return StepOutcome::Progress;
        }
        if self.pending.is_none() {
            if let Err(outcome) = self.request_wait(io) {
                return outcome;
            }
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

struct StateCountBack {
    current: u64,
    initial_emitted: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for StateCountBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.initial_emitted {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send_canonical(
                PortId(0),
                CanonicalValue::new(&self.current.to_le_bytes()).expect("Count is eight bytes"),
            )
            .expect("ready initial Count output");
            self.initial_emitted = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len != conduit_time::TICK_ENCODED_LEN {
                return fail(21);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(current) = self.current.checked_add(1) else {
                return capacity_fail(21);
            };
            io.consume(PortId(0)).expect("present Tick input");
            io.send_canonical(
                PortId(0),
                CanonicalValue::new(&current.to_le_bytes()).expect("Count is eight bytes"),
            )
            .expect("ready Count output");
            self.current = current;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed Tick input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

fn capacity_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::IdentityCapacityExhausted,
        detail,
    })
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostCallOutcome;

    #[test]
    fn time_every_offer_keeps_the_canonical_quantity_startup_contract() {
        let offer = time_every_offer();
        assert_eq!(offer.startup_parameters.len(), 1);
        assert_eq!(
            offer.startup_parameters[0].value_type.as_str(),
            conduit_core::QUANTITY_INFO_ID
        );
        assert!(!offer.startup_parameters[0].has_default);
    }

    #[test]
    fn every_rearms_one_wait_and_emits_tick_five_and_later() {
        let wait = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: conduit_time::TICK_ENCODED_LEN,
        };
        let mut operation = TimeEveryBack {
            wait,
            sequence: 0,
            next_request: 0,
            pending: None,
        };
        let mut start = StepIo::test_frame([None], [false], [Some(8)], None, 4);
        assert_eq!(
            operation.step(&mut start, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            start.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        for sequence in 0..7_u64 {
            let outcome = HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            };
            let mut io = StepIo::test_frame(
                [None],
                [false],
                [Some(conduit_time::TICK_ENCODED_LEN)],
                Some((RequestId(sequence as u32), outcome)),
                5,
            );
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
                StepOutcome::Progress
            );
            let (port, value) = io.test_canonical_output().unwrap();
            assert_eq!(*port, PortId(0));
            assert_eq!(value.as_slice(), conduit_time::encode_tick(sequence));
            assert_eq!(
                io.test_host_request().map(|request| request.0),
                Some(RequestId(sequence as u32 + 1))
            );
        }
    }
}
