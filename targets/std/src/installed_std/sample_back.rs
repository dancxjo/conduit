//! Exact Boolean sampling driven by an ordinary explicit cadence flow.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{InfoBool, PlannedGear, PortDirection, BOOL_ENCODED_LEN};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};

pub(super) static TIME_SAMPLE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TIME_SAMPLE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct SampleBoolBack {
    latest: Option<[u8; BOOL_ENCODED_LEN]>,
    value_closed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for SampleBoolBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            let canonical = input_bytes.input(PortId(0));
            if self.value_closed
                || value.byte_len != BOOL_ENCODED_LEN as u32
                || canonical
                    .and_then(|bytes| InfoBool::decode(bytes).ok())
                    .is_none()
            {
                return StepOutcome::Fail(sample_failure(1));
            }
            io.consume(PortId(0)).expect("present sample value");
            self.latest = Some(
                canonical
                    .expect("validated sample Boolean bytes")
                    .try_into()
                    .expect("validated sample Boolean has exact canonical length"),
            );
            return StepOutcome::Progress;
        }

        if let Some(cadence) = io.input(PortId(1)) {
            let canonical = input_bytes.input(PortId(1));
            if cadence.byte_len != conduit_time::TICK_ENCODED_LEN
                || canonical
                    .and_then(|bytes| conduit_time::decode_tick(bytes).ok())
                    .is_none()
            {
                return StepOutcome::Fail(sample_failure(2));
            }
            if self.latest.is_some() && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(1)).expect("present sample cadence");
            if let Some(latest) = self.latest {
                io.send_canonical(
                    PortId(0),
                    CanonicalValue::new(&latest).expect("Boolean fits derived-value bound"),
                )
                .expect("ready sample output for retained value");
            }
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && !self.value_closed {
            io.consume_closed(PortId(0))
                .expect("observed sample value closure");
            self.value_closed = true;
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(1)) {
            io.consume_closed(PortId(1))
                .expect("observed sample cadence closure");
            self.latest = None;
            return StepOutcome::Complete;
        }

        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.latest = None;
    }
}

fn sample_failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    }
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: conduit_time::TICK_ENCODED_LEN,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::TimeSample(SampleBoolBack {
        latest: None,
        value_closed: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::time_sample_offer();
    if placement.kind_id.as_str() != conduit_semantic_catalog::TIME_SAMPLE_KIND
        || placement.kind_contract_revision
            != conduit_semantic_catalog::TIME_SAMPLE_CONTRACT_REVISION.into()
        || placement.execution_profile_id
            != conduit_std_offers::TIME_SAMPLE_EXECUTION_PROFILE.into()
        || placement.implementation_id != conduit_std_offers::TIME_SAMPLE_IMPLEMENTATION.into()
        || placement.artifact_id != conduit_std_offers::TIME_SAMPLE_ARTIFACT.into()
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || placement
            .inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || placement
            .outputs
            .iter()
            .any(|port| port.direction != PortDirection::Output)
    {
        return Err("planned time/sample identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::scheduler::StepOutcome;
    use conduit_kernel::ValueRef;

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: BOOL_ENCODED_LEN as u32,
        }
    }

    fn tick(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: conduit_time::TICK_ENCODED_LEN,
        }
    }

    #[test]
    fn explicit_cadence_samples_latest_and_value_wins_simultaneous_step() {
        let mut operation = SampleBoolBack {
            latest: None,
            value_closed: false,
        };
        let first_bytes = InfoBool::new(false).encode();
        let tick_bytes = conduit_time::encode_tick(1);
        let mut io = StepIo::test_frame(
            [Some(value(1)), Some(tick(2))],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&first_bytes), Some(&tick_bytes)], None),
            ),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(0)));
        assert!(!io.test_consumed(PortId(1)));

        let mut io = StepIo::test_frame(
            [None, Some(tick(2))],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None, Some(&tick_bytes)], None),
            ),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_canonical_output()
                .map(|(_, value)| value.as_slice()),
            Some(first_bytes.as_slice())
        );

        let second_bytes = InfoBool::new(true).encode();
        let mut io = StepIo::test_frame(
            [Some(value(3)), None],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&second_bytes), None], None),
            ),
            StepOutcome::Progress
        );
        assert_eq!(operation.latest, Some(second_bytes));
    }

    #[test]
    fn cadence_before_value_is_empty_and_cadence_close_releases_latest() {
        let mut operation = SampleBoolBack {
            latest: None,
            value_closed: false,
        };
        let tick_bytes = conduit_time::encode_tick(1);
        let mut io = StepIo::test_frame(
            [None, Some(tick(2))],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None, Some(&tick_bytes)], None),
            ),
            StepOutcome::Progress
        );
        assert!(io.test_output(PortId(0)).is_none());

        operation.latest = Some(InfoBool::new(true).encode());
        let mut io = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        assert!(operation.latest.is_none());
    }

    #[test]
    fn rejects_malformed_boolean_and_tick_payloads() {
        let mut operation = SampleBoolBack {
            latest: None,
            value_closed: false,
        };
        let bad_bool = [2];
        let mut io = StepIo::test_frame(
            [Some(value(1)), None],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert!(matches!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&bad_bool), None], None),
            ),
            StepOutcome::Fail(_)
        ));

        let bad_tick = [0; 7];
        let malformed_tick = ValueRef {
            slot: 2,
            generation: 1,
            byte_len: 7,
        };
        let mut io = StepIo::test_frame(
            [None, Some(malformed_tick)],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert!(matches!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None, Some(&bad_tick)], None),
            ),
            StepOutcome::Fail(_)
        ));
    }
}
