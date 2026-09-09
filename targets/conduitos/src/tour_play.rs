//! Exact Plan and production-kernel preparation for the canonical Tour specimen.

use conduit_tour_model::{CANONICAL_LITERAL, CANONICAL_RESULT, CANONICAL_SOURCE};

use crate::{
    composition::{MachineRunError, MachineRunReceipt},
    identity::BootIdentities,
    machine::{BaseError, IdleBase, InterruptBase, MonotonicClockBase, SerialBase},
    offer::HostOffer,
    ordinary_plan::{PreparationError, PreparedOrdinaryPlay},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourPlayEvidence {
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: conduit_core::ExpandedFormId,
    pub plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub result: &'static str,
    pub run: MachineRunReceipt,
    pub observations: crate::text_composition::TextObservations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourPlayError {
    Machine(MachineRunError),
    ResultMismatch,
    ResultMissing,
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
    let mut exact_serial = ExactTourSerial {
        inner: serial,
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
        source_document_id: prepared.source_document_id.clone(),
        checked_form_id: prepared.checked_form_id.clone(),
        expanded_form_id: prepared.expanded_form_id.clone(),
        plan_id: prepared.plan_id.clone(),
        active_play_id: prepared.active_play.active_play_id.clone(),
        result: CANONICAL_RESULT,
        run,
        observations,
    })
}

struct ExactTourSerial<'a, S> {
    inner: &'a mut S,
    observed: bool,
    mismatch: bool,
}

impl<S: SerialBase> SerialBase for ExactTourSerial<'_, S> {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        if bytes != CANONICAL_RESULT.as_bytes() || self.observed {
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
            observed: false,
            mismatch: false,
        };
        assert_eq!(exact.present(b"WRONG"), Err(BaseError::UnsupportedValue));
        assert!(exact.mismatch && !exact.observed);

        let mut serial = Serial::default();
        let mut exact = ExactTourSerial {
            inner: &mut serial,
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
