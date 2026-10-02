#![no_std]

extern crate alloc;

#[allow(clippy::clone_on_copy, clippy::too_many_arguments, dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));

    impl Copy for KeyModifiers {}

    impl PartialOrd for KeyModifiers {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for KeyModifiers {
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            self.0.cmp(&other.0)
        }
    }

    impl core::hash::Hash for KeyModifiers {
        fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
            self.0.hash(state);
        }
    }

    impl KeyModifiers {
        pub const NONE: Self = Self(0);
        pub const LEFT_CONTROL: Self = Self(1 << 0);
        pub const LEFT_SHIFT: Self = Self(1 << 1);
        pub const LEFT_ALT: Self = Self(1 << 2);
        pub const LEFT_GUI: Self = Self(1 << 3);
        pub const RIGHT_CONTROL: Self = Self(1 << 4);
        pub const RIGHT_SHIFT: Self = Self(1 << 5);
        pub const RIGHT_ALT: Self = Self(1 << 6);
        pub const RIGHT_GUI: Self = Self(1 << 7);

        pub const fn from_bits(bits: u8) -> Self {
            Self(bits)
        }

        pub const fn bits(self) -> u8 {
            self.0
        }

        pub const fn contains_usage(self, usage: u8) -> bool {
            if usage < crate::MODIFIER_USAGE_MINIMUM || usage > crate::MODIFIER_USAGE_MAXIMUM {
                return false;
            }
            self.0 & (1 << (usage - crate::MODIFIER_USAGE_MINIMUM)) != 0
        }
    }
}
pub use generated::{
    ChordInfo, ChordPhase, ChordPhaseCode, ControlChordModifier, CoreChordId, CoreChordIdCode,
    CurrentExperienceInspectionError, CurrentExperienceProjection, ExperienceAvailability,
    ExperienceBodyInput, ExperienceCertainty, ExperienceDomain, ExperienceHumanInput,
    ExperienceHypothesisInput, ExperienceInferenceInput, ExperienceMemoryInput, ExperienceOrigin,
    ExperienceRefusal, ExperienceRelationKind, ExperienceSourceRefusal, ExperienceSourceStatus,
    ExperienceTemporalPolicy, ExperienceTemporalRefusal, ExperienceTemporalRole,
    ExperienceUpdateRefusal, GamepadState, HumanMediaKind, ImageObservationReference,
    ImageObservationRefusal, ImageRegion, ImageTextContentDigest, ImageTextMetadata,
    ImageTextMetadataEntries, ImageTextRecord, ImageTextRefusal, InputAxisSlot, InputAxisSlots,
    InputAxisState, InputButtonPhase, InputButtonSlot, InputButtonSlots, InputButtonState,
    InputButtonTransition, InputPressure, InputPressurePolicy, InputSurfacePoint,
    InputSurfaceVector, InteractionApplicationOutcome, InteractionApplicationOutcomeAccepted,
    InteractionApplicationOutcomeFailed, InteractionApplicationOutcomeRefused,
    InteractionBoundKind as BoundKind, InteractionCanonicalBytes, InteractionFamily,
    InteractionFamilyChooseMany, InteractionFamilyChooseOne, InteractionFamilyRelativeAdjustment,
    InteractionFamilyScalar, InteractionFamilyStructured, InteractionFamilyText,
    InteractionProposalPayload, InteractionRefusal, InteractionTypeDigest, InteractionValue,
    InteractionValueKind, InteractionValues, KeyEvent, KeyModifiers, KeyTransition,
    KeyTransitionCode, KeymapDisposition, KeymapRefusal, OptionAvailability,
    OptionAvailabilityUnavailable, PointerEvent, RealizationRangePolicy, RotaryDirection,
    RotaryStep, ScalarQuantization, SourceAvailability, TextFragment, TouchContact,
    TouchContactPhase, TouchContactSlot, TouchContacts, TouchFrame, VisualEvidenceClass,
    VisualExperienceRefusal, VisualExperienceRelationKind, VisualImpressionDisposition,
    VisualImpressionDispositionTruncated, VisualImpressionRefusal, VisualObservationRefusal,
};

mod current_experience;
mod current_experience_trace;
mod experience_observation;
mod experience_sources;
mod experience_temporal;
mod experience_updates;
mod human_interaction;
mod human_media;
mod image_observation;
mod image_text;
mod image_text_codec;
mod input_chord;
mod input_keymap;
mod key_event;
mod visual_experience;
mod visual_impression;
mod visual_observation;

pub use current_experience::*;
pub use current_experience_trace::*;
pub use experience_observation::*;
pub use experience_sources::*;
pub use experience_updates::*;
pub use human_interaction::*;
pub use human_media::*;
pub use image_observation::*;
pub use image_text::*;
pub use image_text_codec::*;
pub use input_chord::*;
pub use input_keymap::*;
pub use key_event::*;
pub use visual_experience::*;
pub use visual_impression::*;
pub use visual_observation::*;
