//! Root-owned physical providers used by canonical product execution effects.
//! No new scheduler or execution semantics are introduced here. Returning the
//! existing receipt leaves ProductJourney responsible for semantic acceptance.

use super::controller_io::RunTourStage;
use crate::{arch, identity::BootIdentities, offer_make::ImageBoundHostOffer};

pub(super) struct RootRuntime {
    clock: arch::Clock,
    serial: arch::Serial,
    interrupts: arch::Interrupts,
    idle: arch::Idle,
}

impl RootRuntime {
    pub fn new() -> Self {
        Self {
            clock: arch::Clock::new(),
            serial: arch::Serial::new(),
            interrupts: arch::Interrupts::new(),
            idle: arch::Idle::new(),
        }
    }

    pub fn run_tour(
        &mut self,
        identities: &BootIdentities,
        offer: &ImageBoundHostOffer<'_>,
        build_id: &str,
        request: RunTourStage,
    ) -> Result<crate::tour_play::TourPlayEvidence, &'static str> {
        if request
            == (RunTourStage {
                chapter: 0,
                stage: 2,
            })
        {
            let mut prepared = crate::tour_play::prepare_morse_stage(identities, offer, build_id)
                .map_err(|error| error.as_str())?;
            crate::tour_play::run_morse_stage(
                &mut prepared,
                &mut self.clock,
                &mut self.serial,
                &mut self.interrupts,
                &mut self.idle,
            )
            .map_err(|_| "resident-tour-morse-play-refused")
        } else {
            let (mut prepared, specimen_id, expected) = crate::tour_play::prepare_stage(
                identities,
                offer,
                build_id,
                request.chapter,
                request.stage,
            )
            .map_err(|error| error.as_str())?;
            crate::tour_play::run_stage(
                &mut prepared,
                specimen_id,
                expected,
                &mut self.clock,
                &mut self.serial,
                &mut self.interrupts,
                &mut self.idle,
            )
            .map_err(|_| "resident-tour-play-refused")
        }
    }
}
