//! Stateless selection with explicit observation time and freshness policy.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{BoundedExperienceSituation, EXPERIENCE_EVIDENCE_MAX_BYTES};
const FRAME: PortId = PortId(0);
const EVENT: PortId = PortId(0);
const SITUATION: PortId = PortId(1);
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_SITUATION_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct ExperienceSituationBack {
    selector: BoundedExperienceSituation,
    staged: bool,
    complete: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for ExperienceSituationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        let Some(frame) = bytes.input(FRAME) else {
            if io.input_closed(FRAME) {
                if io.consume_closed(FRAME).is_err() {
                    return failure(1);
                }
                self.complete = true;
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        };
        if !io.output_ready(EVENT) || !io.output_ready(SITUATION) {
            return StepOutcome::Await;
        }
        if let Err(reason) = self.selector.execute(frame) {
            return failure(refusal_detail(reason));
        }
        let event = self.selector.event().len() as u32;
        let situation = self.selector.situation().len() as u32;
        if !io.output_ready_for(EVENT, event) || !io.output_ready_for(SITUATION, situation) {
            return StepOutcome::Await;
        }
        if io.consume(FRAME).is_err()
            || io.send_prepared(EVENT, event).is_err()
            || io.send_prepared(SITUATION, situation).is_err()
        {
            return failure(3);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if !self.staged {
            return None;
        }
        match port {
            EVENT => Some(self.selector.event()),
            SITUATION => Some(self.selector.situation()),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.complete = true;
    }
}
fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
fn refusal_detail(reason: conduit_semantic_catalog::ExperienceSituationRefusal) -> u16 {
    use conduit_semantic_catalog::ExperienceSituationRefusal::*;
    match reason {
        Malformed => 10,
        ByteBound => 11,
        SourceBound => 12,
        RememberedInput => 13,
        InvalidAvailability => 14,
        FutureObservation => 15,
        Temporal(conduit_core::TemporalRelationError::InvalidInstant) => 16,
        Temporal(conduit_core::TemporalRelationError::Incomparable) => 17,
        Temporal(conduit_core::TemporalRelationError::IntervalOverflow) => 18,
        Evidence(reason) => 100 + super::experience_evidence_back::refusal_detail(reason),
    }
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::experience_situation_offer(),
    )?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (EXPERIENCE_EVIDENCE_MAX_BYTES * 2) as u32,
        maximum_value_bytes: EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::ExperienceSituation(
        ExperienceSituationBack {
            selector: BoundedExperienceSituation::prepare(),
            staged: false,
            complete: false,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_human::*;
    use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
    fn frame() -> Vec<u8> {
        ExperienceSituationFrame::new(
            ExperienceSituationObservations::new(BoundedSequence::new()).unwrap(),
            ExperienceTemporalPolicy::new(10, 20).unwrap(),
            conduit_time::NativeTemporalInstant::new(
                "clock/pressure".into(),
                1,
                conduit_time::NativeTemporalScale::Milliseconds,
                100,
                0,
            )
            .unwrap(),
            false,
            ExperienceEvidenceSelection::new(
                ExperienceEvidenceIdentities::new(BoundedSequence::new()).unwrap(),
                ExperienceEvidenceIdentity::new("frame/pressure".into()).unwrap(),
                0,
            )
            .unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap()
    }
    fn back() -> ExperienceSituationBack {
        ExperienceSituationBack {
            selector: BoundedExperienceSituation::prepare(),
            staged: false,
            complete: false,
        }
    }
    fn value(bytes: &[u8]) -> conduit_kernel::ValueRef {
        conduit_kernel::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        }
    }
    #[test]
    fn blocked_output_does_not_parse_or_consume_a_frame() {
        let mut back = back();
        let input = b"malformed";
        let bytes = StepInputBytes::test_frame([Some(input.as_slice()), None], None);
        let mut blocked = StepIo::test_frame(
            [Some(value(input)), None],
            [false; 2],
            [Some(65_536), None],
            None,
            8,
        );
        assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
        assert!(!blocked.test_consumed(FRAME));
        let mut ready = StepIo::test_frame(
            [Some(value(input)), None],
            [false; 2],
            [Some(65_536); 2],
            None,
            8,
        );
        assert_eq!(back.step(&mut ready, &bytes), failure(10));
        assert!(!ready.test_consumed(FRAME));
    }
    #[test]
    fn both_typed_outputs_publish_atomically_and_cancel_discards_staging() {
        let mut back = back();
        let input = frame();
        let bytes = StepInputBytes::test_frame([Some(input.as_slice()), None], None);
        let mut small = StepIo::test_frame(
            [Some(value(&input)), None],
            [false; 2],
            [Some(65_536), Some(1)],
            None,
            8,
        );
        assert_eq!(back.step(&mut small, &bytes), StepOutcome::Await);
        assert!(!small.test_consumed(FRAME));
        assert!(StepBack::<2>::prepared_output(&back, EVENT).is_none());
        let mut ready = StepIo::test_frame(
            [Some(value(&input)), None],
            [false; 2],
            [Some(65_536); 2],
            None,
            8,
        );
        assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
        assert!(ready.test_consumed(FRAME));
        let event = ExperienceAutobiographyEvent::decode(
            StepBack::<2>::prepared_output(&back, EVENT).unwrap(),
        )
        .unwrap();
        let situation = ExperienceEvidenceRequest::decode(
            StepBack::<2>::prepared_output(&back, SITUATION).unwrap(),
        )
        .unwrap();
        assert_eq!(event.observation(), &situation);
        StepBack::<2>::cancel(&mut back);
        assert!(StepBack::<2>::prepared_output(&back, EVENT).is_none());
        assert!(StepBack::<2>::prepared_output(&back, SITUATION).is_none());
    }
}
