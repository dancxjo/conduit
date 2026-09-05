//! Canonical discovery metadata for the supported user-facing Kind nucleus.
//!
//! Categories, tags, and icons help people find a Kind. They are deliberately
//! absent from Kind contracts and therefore cannot alter semantic identity.

use conduit_alife::{LENIA_STEP_KIND, ORBIUM_SEED_KIND, SCALAR_FIELD_PRESENTATION_KIND};
use conduit_core::KindId;
pub use conduit_presentation::PresentationIconKey as PaletteIconKey;
use conduit_presentation::BITMAP_PRESENTATION_KIND;

use crate::{
    AUDIO_RENDER_DEMAND_KIND, BOOL_PRESENTATION_KIND, CHORDS_KIND, COPY_FILE_KIND,
    COUNT_PRESENTATION_KIND, GATE_KIND, GRAPHICS_ICON_KIND, GRAPHICS_PRESENTATION_KIND,
    GRAPHICS_RECT_KIND, GRAPHICS_TEXT_KIND, KEYBOARD_KIND, KEYMAP_KIND, KEY_EVENT_TEE_KIND,
    LATEST_KIND, LAYOUT_ALIGN_KIND, LAYOUT_COLUMN_KIND, LAYOUT_INSET_KIND, LAYOUT_ROW_KIND,
    LAYOUT_STACK_KIND, LAYOUT_VIEWPORT_KIND, LOGIC_COMPARE_KIND, LOGIC_NOT_KIND, LOGIC_SELECT_KIND,
    MATH_CLAMP_KIND, MATH_DEADBAND_KIND, MATH_SCALE_KIND, MUSIC_INPUT_KIND, MUSIC_SYNTH_KIND,
    PATCHBAY_CORD_KIND, PATCHBAY_GEAR_FACE_KIND, PATCHBAY_PORT_KIND, PATCHBAY_PRESENTATION_KIND,
    PRESENTATION_BADGE_KIND, PRESENTATION_FRAME_KIND, PRESENTATION_ICON_KIND, QUANTITY_MAP_KIND,
    ROBOTICS_DOCK_KIND, ROBOTICS_DRIVE_DIFFERENTIAL_KIND, ROBOTICS_OBSERVE_ACCELERATION_KIND,
    ROBOTICS_OBSERVE_BATTERY_KIND, ROBOTICS_OBSERVE_BEACON_KIND, ROBOTICS_OBSERVE_BUMP_KIND,
    ROBOTICS_OBSERVE_BUTTONS_KIND, ROBOTICS_OBSERVE_CHARGING_KIND, ROBOTICS_OBSERVE_CLIFF_KIND,
    ROBOTICS_OBSERVE_CONTACT_KIND, ROBOTICS_OBSERVE_IMU_KIND, ROBOTICS_OBSERVE_ODOMETRY_KIND,
    ROBOTICS_OBSERVE_PROXIMITY_KIND, ROBOTICS_OBSERVE_RANGE_KIND, ROBOTICS_OBSERVE_WHEEL_DROP_KIND,
    ROBOTICS_VELOCITY_INTENT_KIND, STATE_COUNT_KIND, STATE_SELECT_KIND, STATE_TOGGLE_KIND,
    TEE_KIND, TEXT_PRESENTATION_KIND, TICK_KIND, TICK_PRESENTATION_KIND, TIME_DEBOUNCE_KIND,
    TIME_DELAY_KIND, TIME_THROTTLE_KIND, TIME_TIMEOUT_KIND,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaletteCategory {
    TimeAndFlow,
    Transform,
    State,
    Presentation,
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
        Self::Presentation,
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
            Self::Presentation => "Presentation",
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
        SCALAR_FIELD_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["alife", "lenia", "scalar-field", "presenter"],
            PaletteIconKey::Presentation,
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
        LAYOUT_VIEWPORT_KIND => metadata(
            PaletteCategory::Presentation,
            &["layout", "viewport", "extent"],
            PaletteIconKey::Presentation,
        ),
        LAYOUT_INSET_KIND => metadata(
            PaletteCategory::Presentation,
            &["layout", "inset", "clip"],
            PaletteIconKey::Presentation,
        ),
        LAYOUT_ROW_KIND => metadata(
            PaletteCategory::Presentation,
            &["layout", "row", "horizontal"],
            PaletteIconKey::Combine,
        ),
        LAYOUT_COLUMN_KIND => metadata(
            PaletteCategory::Presentation,
            &["layout", "column", "vertical"],
            PaletteIconKey::Combine,
        ),
        LAYOUT_STACK_KIND => metadata(
            PaletteCategory::Presentation,
            &["layout", "stack", "overlay"],
            PaletteIconKey::Combine,
        ),
        LAYOUT_ALIGN_KIND => metadata(
            PaletteCategory::Presentation,
            &["layout", "align", "placement"],
            PaletteIconKey::Presentation,
        ),
        PRESENTATION_ICON_KIND => metadata(
            PaletteCategory::Presentation,
            &["presentation", "icon", "accessible"],
            PaletteIconKey::Presentation,
        ),
        PRESENTATION_FRAME_KIND => metadata(
            PaletteCategory::Presentation,
            &["presentation", "frame", "group"],
            PaletteIconKey::Presentation,
        ),
        PRESENTATION_BADGE_KIND => metadata(
            PaletteCategory::Presentation,
            &["presentation", "badge", "status"],
            PaletteIconKey::Presentation,
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
        TICK_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["display", "tick", "indicator"],
            PaletteIconKey::Presentation,
        ),
        BOOL_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["boolean", "state", "present"],
            PaletteIconKey::Presentation,
        ),
        TEXT_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["display", "text", "screen"],
            PaletteIconKey::Presentation,
        ),
        COUNT_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["display", "count", "chart"],
            PaletteIconKey::ChartColumnsIncreasing,
        ),
        GRAPHICS_RECT_KIND => metadata(
            PaletteCategory::Presentation,
            &["graphics", "rectangle", "frame", "clip"],
            PaletteIconKey::Presentation,
        ),
        GRAPHICS_TEXT_KIND => metadata(
            PaletteCategory::Presentation,
            &["graphics", "resolved", "text", "clip"],
            PaletteIconKey::Type,
        ),
        GRAPHICS_ICON_KIND => metadata(
            PaletteCategory::Presentation,
            &["graphics", "resolved", "icon", "clip"],
            PaletteIconKey::Presentation,
        ),
        GRAPHICS_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["graphics", "scene", "manifest", "surface"],
            PaletteIconKey::Presentation,
        ),
        BITMAP_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["graphics", "bitmap", "gray8", "pixels", "manifest"],
            PaletteIconKey::Presentation,
        ),
        PATCHBAY_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["patchbay", "canvas", "portable", "presenter"],
            PaletteIconKey::Presentation,
        ),
        PATCHBAY_GEAR_FACE_KIND => metadata(
            PaletteCategory::Presentation,
            &["patchbay", "gear", "face", "controls"],
            PaletteIconKey::Presentation,
        ),
        PATCHBAY_PORT_KIND => metadata(
            PaletteCategory::Presentation,
            &["patchbay", "port", "typed", "accessible"],
            PaletteIconKey::Combine,
        ),
        PATCHBAY_CORD_KIND => metadata(
            PaletteCategory::Presentation,
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
        _ => return None,
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
        assert_eq!(contracts.len(), 70);
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
