//! Artifact-bound construction of the current x86 boot-scoped Host offer.

use core::ops::Deref;

use crate::{
    fabrication::{
        FabricationRecord, IMPL_KEYBOARD, IMPL_TEXT_LITERAL, IMPL_TEXT_PRESENTATION,
        IMPL_TEXT_UPPER, IMPL_TICK_PRESENTATION, IMPL_TIME_TICK,
    },
    identity::BootIdentities,
    keyboard_offer::KeyboardRealization,
    offer::{CpuFeatures, HostOffer},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageBoundOfferError {
    FabricationInvalid,
    ImplementationNotInImage,
    InvalidDeviceOffer,
}

impl ImageBoundOfferError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FabricationInvalid => "fabrication-record-invalid",
            Self::ImplementationNotInImage => "implementation-not-in-image",
            Self::InvalidDeviceOffer => "invalid-device-offer",
        }
    }
}

pub struct ImageBoundHostOffer<'a> {
    offer: HostOffer<'a>,
    pub profile_id: &'a str,
    pub image_binding: &'a str,
}

impl<'a> Deref for ImageBoundHostOffer<'a> {
    type Target = HostOffer<'a>;

    fn deref(&self) -> &Self::Target {
        &self.offer
    }
}

impl<'a> ImageBoundHostOffer<'a> {
    pub fn into_inner(self) -> HostOffer<'a> {
        self.offer
    }

    pub fn new(
        ids: &BootIdentities,
        fabrication: &'a FabricationRecord,
        cpu_features: CpuFeatures,
        runtime_arena_bytes: u64,
    ) -> Result<Self, ImageBoundOfferError> {
        fabrication
            .validate(runtime_arena_bytes)
            .map_err(|_| ImageBoundOfferError::FabricationInvalid)?;
        let required = IMPL_TIME_TICK
            | IMPL_TICK_PRESENTATION
            | IMPL_TEXT_LITERAL
            | IMPL_TEXT_UPPER
            | IMPL_TEXT_PRESENTATION;
        if !fabrication.includes(required) {
            return Err(ImageBoundOfferError::ImplementationNotInImage);
        }
        Ok(Self {
            offer: HostOffer::new(ids, fabrication.build_id, cpu_features, runtime_arena_bytes),
            profile_id: fabrication.profile_id,
            image_binding: fabrication.image_binding,
        })
    }

    pub fn with_keyboard(
        mut self,
        fabrication: &FabricationRecord,
        realization: KeyboardRealization,
    ) -> Result<Self, ImageBoundOfferError> {
        if !fabrication.includes(IMPL_KEYBOARD) {
            return Err(ImageBoundOfferError::ImplementationNotInImage);
        }
        self.offer = self
            .offer
            .with_keyboard(realization, fabrication.build_id)
            .map_err(|_| ImageBoundOfferError::InvalidDeviceOffer)?;
        Ok(self)
    }

    #[cfg(target_arch = "x86_64")]
    pub fn with_pc_speaker(
        mut self,
        fabrication: &FabricationRecord,
        realization: crate::pc_speaker_offer::PcSpeakerRealization,
    ) -> Result<Self, ImageBoundOfferError> {
        if !fabrication.includes(crate::fabrication::IMPL_PC_SPEAKER) {
            return Err(ImageBoundOfferError::ImplementationNotInImage);
        }
        self.offer = self
            .offer
            .with_pc_speaker(realization, fabrication.build_id)
            .map_err(|_| ImageBoundOfferError::InvalidDeviceOffer)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabrication::{
        ALL_KNOWN_IMPLEMENTATIONS, BASE_DISPLAY_SCANOUT, DRIVER_LINEAR_FRAMEBUFFER,
        FABRICATION_SCHEMA, FACILITY_NATIVE_COMPOSITOR, IMPL_HTTP_CLIENT, IMPL_LINEAR_PRESENTER,
        PRESENTER_NATIVE_GRAPHICAL, RESOURCE_PRESENTATION_SURFACE,
    };

    fn fabrication() -> FabricationRecord {
        FabricationRecord {
            schema: FABRICATION_SCHEMA,
            profile_id: "profile:sha256:test",
            build_id: "build:sha256:test",
            image_binding: "image:sha256:test",
            target: "conduitos/x86_64/pc",
            implementations: ALL_KNOWN_IMPLEMENTATIONS & !IMPL_LINEAR_PRESENTER & !IMPL_HTTP_CLIENT,
            facilities: FACILITY_NATIVE_COMPOSITOR,
            resources: RESOURCE_PRESENTATION_SURFACE,
            bases: BASE_DISPLAY_SCANOUT,
            drivers: DRIVER_LINEAR_FRAMEBUFFER,
            presenters: PRESENTER_NATIVE_GRAPHICAL,
            proof_instrumentation: 0,
            presentation_surface_slots: 2,
            presentation_surface_bytes: 4 * 1024 * 1024,
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
        let offer = ImageBoundHostOffer::new(
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
        assert!(matches!(
            ImageBoundHostOffer::new(&ids, &missing, offer.cpu_features, 262_144),
            Err(ImageBoundOfferError::ImplementationNotInImage)
        ));
    }
}
