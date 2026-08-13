//! Artifact-bound construction of the current boot-scoped Host offer.

use crate::{
    fabrication::{
        FabricationRecord, IMPL_KEYBOARD, IMPL_TEXT_LITERAL, IMPL_TEXT_PRESENTATION,
        IMPL_TEXT_UPPER, IMPL_TICK_PRESENTATION, IMPL_TIME_TICK,
    },
    identity::BootIdentities,
    keyboard_offer::KeyboardRealization,
    offer::{CpuFeatures, HostOffer, OfferError},
};

impl<'a> HostOffer<'a> {
    pub fn new_image_bound(
        ids: &BootIdentities,
        fabrication: &'a FabricationRecord,
        cpu_features: CpuFeatures,
        runtime_arena_bytes: u64,
    ) -> Result<Self, OfferError> {
        fabrication
            .validate(runtime_arena_bytes)
            .map_err(|_| OfferError::FabricationInvalid)?;
        let required = IMPL_TIME_TICK
            | IMPL_TICK_PRESENTATION
            | IMPL_TEXT_LITERAL
            | IMPL_TEXT_UPPER
            | IMPL_TEXT_PRESENTATION;
        if !fabrication.includes(required) {
            return Err(OfferError::ImplementationNotInImage);
        }
        let mut offer = Self::new(ids, fabrication.build_id, cpu_features, runtime_arena_bytes);
        offer.profile_id = fabrication.profile_id;
        offer.image_binding = fabrication.image_binding;
        Ok(offer)
    }

    pub fn with_image_bound_keyboard(
        self,
        fabrication: &FabricationRecord,
        realization: KeyboardRealization,
    ) -> Result<Self, OfferError> {
        if !fabrication.includes(IMPL_KEYBOARD) {
            return Err(OfferError::ImplementationNotInImage);
        }
        self.with_keyboard(realization, fabrication.build_id)
    }

    #[cfg(target_arch = "x86_64")]
    pub fn with_image_bound_pc_speaker(
        self,
        fabrication: &FabricationRecord,
        realization: crate::pc_speaker_offer::PcSpeakerRealization,
    ) -> Result<Self, OfferError> {
        if !fabrication.includes(crate::fabrication::IMPL_PC_SPEAKER) {
            return Err(OfferError::ImplementationNotInImage);
        }
        self.with_pc_speaker(realization, fabrication.build_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabrication::{
        ALL_KNOWN_IMPLEMENTATIONS, FABRICATION_SCHEMA, FACILITY_NATIVE_COMPOSITOR,
    };

    fn fabrication() -> FabricationRecord {
        FabricationRecord {
            schema: FABRICATION_SCHEMA,
            profile_id: "profile:sha256:test",
            build_id: "build:sha256:test",
            image_binding: "image:sha256:test",
            target: "conduitos/x86_64/pc",
            implementations: ALL_KNOWN_IMPLEMENTATIONS,
            facilities: FACILITY_NATIVE_COMPOSITOR,
            runtime_arena_ceiling: 262_144,
            operation_slot_ceiling: 64,
            timer_slot_ceiling: 32,
            evidence_item_ceiling: 64,
        }
    }

    #[test]
    fn image_inventory_and_live_truth_both_bound_the_current_offer() {
        let ids = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let record = fabrication();
        let offer = HostOffer::new_image_bound(
            &ids,
            &record,
            CpuFeatures {
                sse2: true,
                rdrand: false,
                invariant_tsc: true,
            },
            262_144,
        )
        .unwrap();
        assert_eq!(offer.profile_id, record.profile_id);
        assert_eq!(offer.image_binding, record.image_binding);
        assert!(record.includes(IMPL_KEYBOARD));
        assert!(offer.keyboard.is_none());
        let missing = FabricationRecord {
            implementations: record.implementations & !IMPL_TEXT_UPPER,
            ..record
        };
        assert_eq!(
            HostOffer::new_image_bound(&ids, &missing, offer.cpu_features, 262_144),
            Err(OfferError::ImplementationNotInImage)
        );
    }
}
