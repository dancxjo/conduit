//! Exact Plan and production-kernel preparation for the canonical Tour specimen.

use conduit_tour_model::{
    CANONICAL_LITERAL, CANONICAL_RESULT, CANONICAL_SOURCE, CANONICAL_SPECIMEN_ID, TOUR_CHAPTERS,
    TourStageMode, tour_stage_source,
};

use crate::{
    composition::{MachineRunError, MachineRunReceipt},
    identity::BootIdentities,
    machine::{BaseError, IdleBase, InterruptBase, MonotonicClockBase, SerialBase},
    offer::HostOffer,
    ordinary_plan::{PreparationError, PreparedOrdinaryPlay},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourPlayEvidence {
    pub specimen_id: &'static str,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: conduit_core::ExpandedFormId,
    pub plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub result: &'static str,
    pub manifestations: u8,
    pub comparison_expanded_form_id: Option<conduit_core::ExpandedFormId>,
    pub comparison_plan_id: Option<conduit_core::PlanId>,
    pub multi_host: Option<conduit_tour_model::TourMultiHostProof>,
    pub terminal: conduit_tour_model::TourRunTerminal,
    pub run: MachineRunReceipt,
    pub observations: crate::text_composition::TextObservations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourPlayError {
    Machine(MachineRunError),
    ResultMismatch,
    ResultMissing,
}

impl TourPlayError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Machine(error) => error.as_str(),
            Self::ResultMismatch => "tour-result-mismatch",
            Self::ResultMissing => "tour-result-missing",
        }
    }
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedOrdinaryPlay, PreparationError> {
    crate::ordinary_plan::prepare_source(
        identities,
        offer,
        build_id,
        CANONICAL_SOURCE,
        "meet-one-gear",
        CANONICAL_LITERAL,
    )
}

pub fn prepare_stage(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
    chapter: u8,
    stage: u8,
) -> Result<(PreparedOrdinaryPlay, &'static str, &'static str), PreparationError> {
    let contract = TOUR_CHAPTERS
        .get(usize::from(chapter))
        .and_then(|chapter| chapter.stages.get(usize::from(stage)))
        .ok_or(PreparationError::FormRejected)?;
    if contract.mode != TourStageMode::Run || chapter != 0 || stage > 1 {
        return Err(PreparationError::FormRejected);
    }
    let source = tour_stage_source(chapter, stage).map_err(|_| PreparationError::FormRejected)?;
    let form_name = contract
        .identity
        .strip_prefix("canonical-form:")
        .ok_or(PreparationError::FormRejected)?;
    let literal = match stage {
        0 => CANONICAL_LITERAL,
        1 => "make this loud",
        _ => return Err(PreparationError::FormRejected),
    };
    let expected = contract
        .expected_text
        .ok_or(PreparationError::FormRejected)?;
    crate::ordinary_plan::prepare_source(identities, offer, build_id, &source, form_name, literal)
        .map(|prepared| (prepared, contract.identity, expected))
}

pub fn run<C, S, I, D>(
    prepared: &mut PreparedOrdinaryPlay,
    clock: &mut C,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<TourPlayEvidence, TourPlayError>
where
    C: MonotonicClockBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    run_stage(
        prepared,
        CANONICAL_SPECIMEN_ID,
        CANONICAL_RESULT,
        clock,
        serial,
        interrupts,
        idle,
    )
}

pub fn run_stage<C, S, I, D>(
    prepared: &mut PreparedOrdinaryPlay,
    specimen_id: &'static str,
    expected: &'static str,
    clock: &mut C,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<TourPlayEvidence, TourPlayError>
where
    C: MonotonicClockBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let mut exact_serial = ExactTourSerial {
        inner: serial,
        expected,
        observed: false,
        mismatch: false,
    };
    let mut observations = crate::text_composition::TextObservations::default();
    let run = crate::text_composition::run_observed(
        &mut prepared.kernel,
        clock,
        &mut exact_serial,
        interrupts,
        idle,
        &mut observations,
    );
    if exact_serial.mismatch {
        return Err(TourPlayError::ResultMismatch);
    }
    let run = run.map_err(TourPlayError::Machine)?;
    if !exact_serial.observed {
        return Err(TourPlayError::ResultMissing);
    }
    Ok(TourPlayEvidence {
        specimen_id,
        source_document_id: prepared.source_document_id.clone(),
        checked_form_id: prepared.checked_form_id.clone(),
        expanded_form_id: prepared.expanded_form_id.clone(),
        plan_id: prepared.plan_id.clone(),
        active_play_id: prepared.active_play.active_play_id.clone(),
        result: expected,
        manifestations: 1,
        comparison_expanded_form_id: None,
        comparison_plan_id: None,
        multi_host: None,
        terminal: conduit_tour_model::TourRunTerminal::Completed,
        run,
        observations,
    })
}

pub fn prepare_morse_stage(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<crate::tour_morse_plan::PreparedTourMorsePlay, PreparationError> {
    crate::tour_morse_plan::prepare(identities, offer, build_id)
}

pub fn run_morse_stage<C, S, I, D>(
    prepared: &mut crate::tour_morse_plan::PreparedTourMorsePlay,
    clock: &mut C,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<TourPlayEvidence, TourPlayError>
where
    C: MonotonicClockBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let run = crate::tour_morse_play::run(prepared, clock, serial, interrupts, idle)
        .map_err(TourPlayError::Machine)?;
    Ok(TourPlayEvidence {
        specimen_id: "canonical-form:branch-a-cord",
        source_document_id: prepared.source_document_id.clone(),
        checked_form_id: prepared.checked_form_id.clone(),
        expanded_form_id: prepared.expanded_form_id.clone(),
        plan_id: prepared.plan_id.clone(),
        active_play_id: prepared.active_play.active_play_id.clone(),
        result: "SOS",
        manifestations: 2,
        comparison_expanded_form_id: None,
        comparison_plan_id: None,
        multi_host: None,
        terminal: conduit_tour_model::TourRunTerminal::Completed,
        run,
        observations: crate::text_composition::TextObservations::default(),
    })
}

pub fn prepare_comparison_stage(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<crate::tour_comparison_plan::PreparedComparisonPlans, PreparationError> {
    crate::tour_comparison_plan::prepare(identities, offer, build_id)
}

pub fn run_comparison_stage<C, S, I, D>(
    prepared: &mut crate::tour_comparison_plan::PreparedComparisonPlans,
    clock: &mut C,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<TourPlayEvidence, TourPlayError>
where
    C: MonotonicClockBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let run = crate::tour_comparison_play::run(prepared, clock, serial, interrupts, idle)
        .map_err(TourPlayError::Machine)?;
    Ok(TourPlayEvidence {
        specimen_id: "canonical-form:same-morse-caller",
        source_document_id: prepared.direct.source_document_id.clone(),
        checked_form_id: prepared.direct.checked_form_id.clone(),
        expanded_form_id: prepared.direct.expanded_form_id.clone(),
        plan_id: prepared.direct.plan_id.clone(),
        active_play_id: prepared.direct_active.active_play_id.clone(),
        result: "Direct and recursive realizations agree",
        manifestations: 2,
        comparison_expanded_form_id: Some(prepared.recursive.expanded_form_id.clone()),
        comparison_plan_id: Some(prepared.recursive.plan_id.clone()),
        multi_host: None,
        terminal: conduit_tour_model::TourRunTerminal::Completed,
        run,
        observations: crate::text_composition::TextObservations::default(),
    })
}

pub fn prepare_timer_stage(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<crate::tour_timer_plan::PreparedTourTimerPlan, PreparationError> {
    crate::tour_timer_plan::prepare(identities, offer, build_id)
}

pub fn run_timer_stage<C, T, S, I, D>(
    prepared: &mut crate::tour_timer_plan::PreparedTourTimerPlan,
    clock: &mut C,
    timer: &mut T,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<TourPlayEvidence, TourPlayError>
where
    C: MonotonicClockBase,
    T: crate::machine::TimerBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let run = crate::tour_timer_play::run(prepared, clock, timer, serial, interrupts, idle)
        .map_err(TourPlayError::Machine)?;
    Ok(TourPlayEvidence {
        specimen_id: "canonical-form:count-over-time",
        source_document_id: prepared.plan.source_document_id.clone(),
        checked_form_id: prepared.plan.checked_form_id.clone(),
        expanded_form_id: prepared.plan.expanded_form_id.clone(),
        plan_id: prepared.plan.plan_id.clone(),
        active_play_id: prepared.active_play.active_play_id.clone(),
        result: "1",
        manifestations: 2,
        comparison_expanded_form_id: None,
        comparison_plan_id: None,
        multi_host: None,
        terminal: conduit_tour_model::TourRunTerminal::Stopped,
        run,
        observations: crate::text_composition::TextObservations::default(),
    })
}

struct ExactTourSerial<'a, S> {
    inner: &'a mut S,
    expected: &'static str,
    observed: bool,
    mismatch: bool,
}

impl<S: SerialBase> SerialBase for ExactTourSerial<'_, S> {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        if bytes != self.expected.as_bytes() || self.observed {
            self.mismatch = true;
            return Err(BaseError::UnsupportedValue);
        }
        self.inner.present(bytes)?;
        self.observed = true;
        Ok(())
    }

    fn presentation_count(&self) -> u32 {
        self.inner.presentation_count()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use crate::{
        machine::{
            BaseError, IdleBase, InterruptBase, InterruptState, MonotonicClockBase, SerialBase,
        },
        offer::CpuFeatures,
    };

    #[derive(Default)]
    struct Clock(u64);
    impl MonotonicClockBase for Clock {
        fn now(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }
    }

    #[derive(Default)]
    struct Serial(Vec<Vec<u8>>);
    impl SerialBase for Serial {
        fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
            self.0.push(bytes.into());
            Ok(())
        }
        fn presentation_count(&self) -> u32 {
            self.0.len() as u32
        }
    }

    #[derive(Default)]
    struct Interrupts(bool);
    impl InterruptBase for Interrupts {
        fn enable(&mut self) {
            self.0 = true;
        }
        fn disable(&mut self) -> InterruptState {
            let state = InterruptState { enabled: self.0 };
            self.0 = false;
            state
        }
        fn restore(&mut self, state: InterruptState) {
            self.0 = state.enabled;
        }
        fn is_enabled(&self) -> bool {
            self.0
        }
    }

    #[derive(Default)]
    struct Idle(u32);
    impl IdleBase for Idle {
        fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
            self.0 += 1;
            Ok(())
        }
        fn idle_count(&self) -> u32 {
            self.0
        }
    }

    fn fixture() -> (BootIdentities, HostOffer<'static>) {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        (identities, offer)
    }

    #[test]
    fn canonical_tour_form_plans_and_runs_on_the_production_kernel() {
        let (identities, offer) = fixture();
        let mut prepared = prepare(&identities, &offer, "build").unwrap();
        assert!(conduit_core::verify_plan(&prepared.plan));
        assert_eq!(
            prepared.plan.source_document_id,
            prepared.source_document_id
        );
        assert_eq!(prepared.plan.checked_form_id, prepared.checked_form_id);
        assert_eq!(prepared.plan.expanded_form_id, prepared.expanded_form_id);
        assert_eq!(
            prepared.plan.completion_policy,
            conduit_core::PlanCompletionPolicy::SemanticCompletion
        );

        let mut clock = Clock::default();
        let mut serial = Serial::default();
        let mut interrupts = Interrupts::default();
        let mut idle = Idle::default();
        let evidence = run(
            &mut prepared,
            &mut clock,
            &mut serial,
            &mut interrupts,
            &mut idle,
        )
        .unwrap();
        assert_eq!(serial.0, [CANONICAL_RESULT.as_bytes()]);
        assert_eq!(
            evidence.source_document_id,
            prepared.plan.source_document_id
        );
        assert_eq!(evidence.checked_form_id, prepared.plan.checked_form_id);
        assert_eq!(evidence.expanded_form_id, prepared.plan.expanded_form_id);
        assert_eq!(evidence.plan_id, prepared.plan.plan_id);
        assert_eq!(evidence.active_play_id, prepared.active_play.active_play_id);
        assert_eq!(evidence.result, CANONICAL_RESULT);
        assert_eq!(evidence.run.logical_operations, 3);
        assert_eq!(evidence.run.serial_presentations, 1);
        assert_eq!(evidence.run.pending_host_operations, 0);
    }

    #[test]
    fn wrong_literal_binding_is_refused_before_play() {
        let (identities, offer) = fixture();
        assert_eq!(
            crate::ordinary_plan::prepare_source(
                &identities,
                &offer,
                "build",
                CANONICAL_SOURCE,
                "meet-one-gear",
                "different",
            )
            .err(),
            Some(PreparationError::KernelRejected)
        );
    }

    #[test]
    fn result_correlation_refuses_wrong_or_duplicate_presentations() {
        let mut serial = Serial::default();
        let mut exact = ExactTourSerial {
            inner: &mut serial,
            expected: CANONICAL_RESULT,
            observed: false,
            mismatch: false,
        };
        assert_eq!(exact.present(b"WRONG"), Err(BaseError::UnsupportedValue));
        assert!(exact.mismatch && !exact.observed);

        let mut serial = Serial::default();
        let mut exact = ExactTourSerial {
            inner: &mut serial,
            expected: CANONICAL_RESULT,
            observed: false,
            mismatch: false,
        };
        assert_eq!(exact.present(CANONICAL_RESULT.as_bytes()), Ok(()));
        assert_eq!(
            exact.present(CANONICAL_RESULT.as_bytes()),
            Err(BaseError::UnsupportedValue)
        );
        assert!(exact.mismatch && exact.observed);
    }
}
