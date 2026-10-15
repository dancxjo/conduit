//! Canonical discovery metadata for the supported user-facing Kind nucleus.
//!
//! Categories, tags, and icons help people find a kind. They are deliberately
//! absent from Kind contracts and therefore cannot alter semantic identity.

use conduit_alife::{LENIA_STEP_KIND, ORBIUM_SEED_KIND, SCALAR_FIELD_FACE_KIND};
use conduit_core::KindId;
pub use conduit_face::FaceIconKey as PaletteIconKey;
use conduit_face::BITMAP_FACE_KIND;

use crate::{
    APPLICATION_EVENT_SOURCE_KIND, APPLICATION_VIEW_FACE_KIND, AUDIO_CONTINUOUS_TONE_KIND,
    AUDIO_GAIN_KIND, AUDIO_RENDER_DEMAND_KIND, AUDIO_TONE_KIND, BOOL_FACE_KIND, CHORDS_KIND,
    COPY_FILE_KIND, COUNT_FACE_KIND, DISTANCE_FREQUENCY_MAP_KIND, FACE_BADGE_KIND, FACE_FRAME_KIND,
    FACE_ICON_KIND, FIRST_KIND, GATE_KIND, GRAPHICS_FACE_KIND, GRAPHICS_ICON_KIND,
    GRAPHICS_RECT_KIND, GRAPHICS_TEXT_KIND, KEYBOARD_KIND, KEYMAP_KIND, KEY_EVENT_TEE_KIND,
    LATEST_KIND, LAYOUT_ALIGN_KIND, LAYOUT_COLUMN_KIND, LAYOUT_INSET_KIND, LAYOUT_ROW_KIND,
    LAYOUT_STACK_KIND, LAYOUT_VIEWPORT_KIND, LOGIC_COMPARE_KIND, LOGIC_NOT_KIND, LOGIC_SELECT_KIND,
    MATH_CLAMP_KIND, MATH_DEADBAND_KIND, MATH_SCALE_KIND, MUSIC_INPUT_KIND, MUSIC_SYNTH_KIND,
    NORMALIZED_DISTANCE_MAP_KIND, PATCHBAY_CORD_KIND, PATCHBAY_FACE_KIND, PATCHBAY_GEAR_FACE_KIND,
    PATCHBAY_PORT_KIND, QUANTITY_MAP_KIND, RETAINED_APPLICATION_KIND, ROBOTICS_DOCK_KIND,
    ROBOTICS_DRIVE_DIFFERENTIAL_KIND, ROBOTICS_OBSERVE_ACCELERATION_KIND,
    ROBOTICS_OBSERVE_BATTERY_KIND, ROBOTICS_OBSERVE_BEACON_KIND, ROBOTICS_OBSERVE_BUMP_KIND,
    ROBOTICS_OBSERVE_BUTTONS_KIND, ROBOTICS_OBSERVE_CHARGING_KIND, ROBOTICS_OBSERVE_CLIFF_KIND,
    ROBOTICS_OBSERVE_CONTACT_KIND, ROBOTICS_OBSERVE_IMU_KIND, ROBOTICS_OBSERVE_ODOMETRY_KIND,
    ROBOTICS_OBSERVE_PROXIMITY_KIND, ROBOTICS_OBSERVE_RANGE_KIND, ROBOTICS_OBSERVE_WHEEL_DROP_KIND,
    ROBOTICS_VELOCITY_INTENT_KIND, STATE_COUNT_KIND, STATE_SELECT_KIND, STATE_TOGGLE_KIND,
    TEE_KIND, TEXT_FACE_KIND, TICK_FACE_KIND, TICK_KIND, TIME_DEADLINE_KIND, TIME_DEBOUNCE_KIND,
    TIME_DELAY_KIND, TIME_THROTTLE_KIND, TIME_TIMEOUT_KIND,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaletteCategory {
    TimeAndFlow,
    Transform,
    State,
    Face,
    Files,
    Robotics,
    Input,
    Protocol,
}

impl PaletteCategory {
    pub const ALL: [Self; 8] = [
        Self::TimeAndFlow,
        Self::Transform,
        Self::State,
        Self::Face,
        Self::Files,
        Self::Robotics,
        Self::Input,
        Self::Protocol,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::TimeAndFlow => "Time & Flow",
            Self::Transform => "Transform",
            Self::State => "State",
            Self::Face => "Face",
            Self::Files => "Files",
            Self::Robotics => "Robotics",
            Self::Input => "Input",
            Self::Protocol => "Protocol",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteMetadata {
    pub category: PaletteCategory,
    pub tags: &'static [&'static str],
    pub icon: PaletteIconKey,
}

pub fn palette_metadata(kind_id: &KindId) -> Option<PaletteMetadata> {
    let metadata = match kind_id.as_str() {
        APPLICATION_EVENT_SOURCE_KIND => metadata(
            PaletteCategory::Input,
            &["application", "event", "interaction", "revision"],
            PaletteIconKey::Port,
        ),
        RETAINED_APPLICATION_KIND => metadata(
            PaletteCategory::State,
            &["application", "retained", "state", "semantic"],
            PaletteIconKey::Body,
        ),
        APPLICATION_VIEW_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["application", "view", "semantic", "face"],
            PaletteIconKey::Face,
        ),
        TICK_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "clock", "pulse"],
            PaletteIconKey::Clock,
        ),
        conduit_time::TIME_EVERY_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "interval", "repeat"],
            PaletteIconKey::Repeat2,
        ),
        AUDIO_RENDER_DEMAND_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["audio", "render", "interval", "clock"],
            PaletteIconKey::Clock,
        ),
        AUDIO_TONE_KIND => metadata(
            PaletteCategory::Transform,
            &["audio", "tone", "frequency", "pcm"],
            PaletteIconKey::Type,
        ),
        AUDIO_CONTINUOUS_TONE_KIND => metadata(
            PaletteCategory::Transform,
            &["audio", "continuous", "tone", "frequency", "pcm"],
            PaletteIconKey::Type,
        ),
        AUDIO_GAIN_KIND => metadata(
            PaletteCategory::Transform,
            &["audio", "gain", "volume", "pcm"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        MUSIC_SYNTH_KIND => metadata(
            PaletteCategory::Transform,
            &["music", "synthesizer", "pcm", "polyphonic"],
            PaletteIconKey::Type,
        ),
        MUSIC_INPUT_KIND => metadata(
            PaletteCategory::Input,
            &["input", "music", "notes", "controls"],
            PaletteIconKey::Keyboard,
        ),
        ORBIUM_SEED_KIND => metadata(
            PaletteCategory::Transform,
            &["alife", "lenia", "orbium", "seed", "scalar-field"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        LENIA_STEP_KIND => metadata(
            PaletteCategory::Transform,
            &["alife", "lenia", "evolve", "scalar-field"],
            PaletteIconKey::Repeat2,
        ),
        SCALAR_FIELD_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["alife", "lenia", "scalar-field", "presenter"],
            PaletteIconKey::Face,
        ),
        TIME_DEBOUNCE_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "debounce", "stable", "robot"],
            PaletteIconKey::Clock,
        ),
        TIME_TIMEOUT_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "timeout", "heartbeat", "safety"],
            PaletteIconKey::Clock,
        ),
        TIME_DELAY_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "delay", "pace", "ordered"],
            PaletteIconKey::Clock,
        ),
        TIME_THROTTLE_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "throttle", "pace", "leading"],
            PaletteIconKey::Clock,
        ),
        TIME_DEADLINE_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "deadline", "cancellation", "request"],
            PaletteIconKey::Clock,
        ),
        KEYBOARD_KIND => metadata(
            PaletteCategory::Input,
            &["input", "keyboard", "key", "source"],
            PaletteIconKey::Keyboard,
        ),
        KEY_EVENT_TEE_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["input", "keyboard", "split", "fan-out"],
            PaletteIconKey::Combine,
        ),
        KEYMAP_KIND => metadata(
            PaletteCategory::Input,
            &["input", "keyboard", "unicode", "conduit-intl"],
            PaletteIconKey::CaseUpper,
        ),
        CHORDS_KIND => metadata(
            PaletteCategory::Input,
            &["input", "keyboard", "modifier", "command"],
            PaletteIconKey::Keyboard,
        ),
        crate::SCALAR_LITERAL_KIND => metadata(
            PaletteCategory::Input,
            &["scalar", "literal", "constant", "value"],
            PaletteIconKey::Type,
        ),
        conduit_text::TEXT_LITERAL_KIND => metadata(
            PaletteCategory::Transform,
            &["source", "constant", "string"],
            PaletteIconKey::Type,
        ),
        conduit_text::TEXT_UPPER_KIND => metadata(
            PaletteCategory::Transform,
            &["uppercase", "case", "string"],
            PaletteIconKey::CaseUpper,
        ),
        conduit_text::TEXT_JOIN_KIND => metadata(
            PaletteCategory::Transform,
            &["prefix", "join", "combine"],
            PaletteIconKey::Combine,
        ),
        STATE_COUNT_KIND => metadata(
            PaletteCategory::State,
            &["counter", "total", "accumulate"],
            PaletteIconKey::Tally5,
        ),
        STATE_TOGGLE_KIND => metadata(
            PaletteCategory::State,
            &["toggle", "boolean", "state"],
            PaletteIconKey::Repeat2,
        ),
        LATEST_KIND => metadata(
            PaletteCategory::State,
            &["latest", "current", "replace"],
            PaletteIconKey::Tally5,
        ),
        TEE_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["split", "tee", "fan-out"],
            PaletteIconKey::Combine,
        ),
        FIRST_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["first", "race", "arbitrate", "cancel"],
            PaletteIconKey::Combine,
        ),
        GATE_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["gate", "enable", "conditional"],
            PaletteIconKey::Combine,
        ),
        STATE_SELECT_KIND => metadata(
            PaletteCategory::State,
            &["select", "current", "choice"],
            PaletteIconKey::Combine,
        ),
        LOGIC_COMPARE_KIND => metadata(
            PaletteCategory::Transform,
            &["compare", "scalar", "decision"],
            PaletteIconKey::Combine,
        ),
        LOGIC_NOT_KIND => metadata(
            PaletteCategory::Transform,
            &["not", "boolean", "invert"],
            PaletteIconKey::Repeat2,
        ),
        LOGIC_SELECT_KIND => metadata(
            PaletteCategory::Transform,
            &["select", "boolean", "choice"],
            PaletteIconKey::Combine,
        ),
        MATH_CLAMP_KIND => metadata(
            PaletteCategory::Transform,
            &["clamp", "limit", "scalar"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        MATH_SCALE_KIND => metadata(
            PaletteCategory::Transform,
            &["scale", "gain", "multiply"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        MATH_DEADBAND_KIND => metadata(
            PaletteCategory::Transform,
            &["deadband", "neutral", "joystick"],
            PaletteIconKey::Combine,
        ),
        QUANTITY_MAP_KIND => metadata(
            PaletteCategory::Transform,
            &["map", "quantity", "unit", "range"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        NORMALIZED_DISTANCE_MAP_KIND => metadata(
            PaletteCategory::Transform,
            &["map", "normalized", "distance", "range"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        DISTANCE_FREQUENCY_MAP_KIND => metadata(
            PaletteCategory::Transform,
            &["map", "distance", "frequency", "theremin"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        crate::QUANTITY_INFO_WRAP_KIND => metadata(
            PaletteCategory::Transform,
            &["quantity", "structured", "leaf", "wrap"],
            PaletteIconKey::Combine,
        ),
        LAYOUT_VIEWPORT_KIND => metadata(
            PaletteCategory::Face,
            &["layout", "viewport", "extent"],
            PaletteIconKey::Face,
        ),
        LAYOUT_INSET_KIND => metadata(
            PaletteCategory::Face,
            &["layout", "inset", "clip"],
            PaletteIconKey::Face,
        ),
        LAYOUT_ROW_KIND => metadata(
            PaletteCategory::Face,
            &["layout", "row", "horizontal"],
            PaletteIconKey::Combine,
        ),
        LAYOUT_COLUMN_KIND => metadata(
            PaletteCategory::Face,
            &["layout", "column", "vertical"],
            PaletteIconKey::Combine,
        ),
        LAYOUT_STACK_KIND => metadata(
            PaletteCategory::Face,
            &["layout", "stack", "overlay"],
            PaletteIconKey::Combine,
        ),
        LAYOUT_ALIGN_KIND => metadata(
            PaletteCategory::Face,
            &["layout", "align", "placement"],
            PaletteIconKey::Face,
        ),
        FACE_ICON_KIND => metadata(
            PaletteCategory::Face,
            &["face", "icon", "accessible"],
            PaletteIconKey::Face,
        ),
        FACE_FRAME_KIND => metadata(
            PaletteCategory::Face,
            &["face", "frame", "group"],
            PaletteIconKey::Face,
        ),
        FACE_BADGE_KIND => metadata(
            PaletteCategory::Face,
            &["face", "badge", "status"],
            PaletteIconKey::Face,
        ),
        ROBOTICS_OBSERVE_BUMP_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "bumper", "contact", "safety"],
            PaletteIconKey::Combine,
        ),
        ROBOTICS_OBSERVE_IMU_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "imu", "orientation", "sensor"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_RANGE_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "range", "distance", "sensor"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_ODOMETRY_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "odometry", "pose", "motion"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_BATTERY_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "battery", "charge", "power"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_VELOCITY_INTENT_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "velocity", "intent", "motion"],
            PaletteIconKey::Combine,
        ),
        ROBOTICS_DRIVE_DIFFERENTIAL_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "drive", "differential", "prewake"],
            PaletteIconKey::Combine,
        ),
        ROBOTICS_OBSERVE_CONTACT_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "contact", "sector", "safety"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_CLIFF_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "cliff", "hazard", "safety"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_WHEEL_DROP_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "wheel", "drop", "safety"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_CHARGING_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "charging", "battery", "power"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_DOCK_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "dock", "charging", "motion"],
            PaletteIconKey::Combine,
        ),
        ROBOTICS_OBSERVE_PROXIMITY_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "proximity", "wall", "sector"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_BEACON_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "beacon", "infrared", "virtual-wall"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        ROBOTICS_OBSERVE_BUTTONS_KIND => metadata(
            PaletteCategory::Input,
            &["robot", "input", "button", "pressed"],
            PaletteIconKey::Keyboard,
        ),
        ROBOTICS_OBSERVE_ACCELERATION_KIND => metadata(
            PaletteCategory::Robotics,
            &["robot", "acceleration", "imu", "impact"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        TICK_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["display", "tick", "indicator"],
            PaletteIconKey::Face,
        ),
        BOOL_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["boolean", "state", "present"],
            PaletteIconKey::Face,
        ),
        TEXT_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["display", "text", "screen"],
            PaletteIconKey::Face,
        ),
        COUNT_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["display", "count", "chart"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        GRAPHICS_RECT_KIND => metadata(
            PaletteCategory::Face,
            &["graphics", "rectangle", "frame", "clip"],
            PaletteIconKey::Face,
        ),
        GRAPHICS_TEXT_KIND => metadata(
            PaletteCategory::Face,
            &["graphics", "resolved", "text", "clip"],
            PaletteIconKey::Type,
        ),
        GRAPHICS_ICON_KIND => metadata(
            PaletteCategory::Face,
            &["graphics", "resolved", "icon", "clip"],
            PaletteIconKey::Face,
        ),
        GRAPHICS_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["graphics", "scene", "manifest", "surface"],
            PaletteIconKey::Face,
        ),
        BITMAP_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["graphics", "bitmap", "gray8", "pixels", "manifest"],
            PaletteIconKey::Face,
        ),
        PATCHBAY_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["patchbay", "canvas", "portable", "presenter"],
            PaletteIconKey::Face,
        ),
        PATCHBAY_GEAR_FACE_KIND => metadata(
            PaletteCategory::Face,
            &["patchbay", "gear", "front", "controls"],
            PaletteIconKey::Face,
        ),
        PATCHBAY_PORT_KIND => metadata(
            PaletteCategory::Face,
            &["patchbay", "port", "typed", "accessible"],
            PaletteIconKey::Combine,
        ),
        PATCHBAY_CORD_KIND => metadata(
            PaletteCategory::Face,
            &["patchbay", "cord", "connection", "line"],
            PaletteIconKey::Combine,
        ),
        COPY_FILE_KIND => metadata(
            PaletteCategory::Files,
            &["copy", "filesystem", "resource"],
            PaletteIconKey::FileOutput,
        ),
        conduit_web::HTTP_CLIENT_KIND => metadata(
            PaletteCategory::Protocol,
            &["http", "request", "client", "protocol"],
            PaletteIconKey::Combine,
        ),
        conduit_web::HTTP_SERVER_KIND => metadata(
            PaletteCategory::Protocol,
            &["http", "response", "server", "protocol"],
            PaletteIconKey::Combine,
        ),
        conduit_web::JSON_ENCODE_KIND => metadata(
            PaletteCategory::Protocol,
            &["json", "encode", "serialize", "protocol"],
            PaletteIconKey::Combine,
        ),
        conduit_web::JSON_DECODE_KIND => metadata(
            PaletteCategory::Protocol,
            &["json", "decode", "parse", "protocol"],
            PaletteIconKey::Combine,
        ),
        _ => return crate::audio_analysis::audio_analysis_palette_metadata(kind_id),
    };
    Some(metadata)
}

const fn metadata(
    category: PaletteCategory,
    tags: &'static [&'static str],
    icon: PaletteIconKey,
) -> PaletteMetadata {
    PaletteMetadata {
        category,
        tags,
        icon,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    #[test]
    fn every_supported_kind_has_non_fallback_legibility_metadata() {
        let contracts = crate::palette_contracts();
        assert_eq!(
            contracts
                .iter()
                .map(|contract| &contract.kind_id)
                .collect::<BTreeSet<_>>()
                .len(),
            contracts.len()
        );
        for contract in contracts {
            let metadata = palette_metadata(&contract.kind_id).expect("palette metadata");
            assert!(!metadata.tags.is_empty());
            assert!(!metadata.icon.is_fallback());
        }
    }

    #[test]
    fn categories_and_upstream_icon_keys_are_finite_and_unique() {
        assert_eq!(PaletteCategory::ALL.len(), 8);
        let keys = PaletteIconKey::ALL_UPSTREAM
            .iter()
            .map(|key| key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), PaletteIconKey::ALL_UPSTREAM.len());
    }
}
