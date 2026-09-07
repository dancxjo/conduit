//! Exact Plan and production-kernel preparation for the canonical Tour specimen.

use conduit_tour_model::{CANONICAL_LITERAL, CANONICAL_SOURCE};

use crate::{
    identity::BootIdentities,
    offer::HostOffer,
    ordinary_plan::{PreparationError, PreparedOrdinaryPlay},
};

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

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use conduit_tour_model::CANONICAL_RESULT;

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
        let receipt = crate::text_composition::run(
            &mut prepared.kernel,
            &mut clock,
            &mut serial,
            &mut interrupts,
            &mut idle,
        )
        .unwrap();
        assert_eq!(serial.0, [CANONICAL_RESULT.as_bytes()]);
        assert_eq!(receipt.logical_operations, 3);
        assert_eq!(receipt.serial_presentations, 1);
        assert_eq!(receipt.pending_host_operations, 0);
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
}
