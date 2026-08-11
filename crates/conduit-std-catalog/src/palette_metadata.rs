//! Canonical discovery metadata for the supported user-facing Kind nucleus.
//!
//! Categories, tags, and icons help people find a Kind. They are deliberately
//! absent from Kind contracts and therefore cannot alter semantic identity.

use conduit_core::KindId;

use crate::{
    BOOL_PRESENTATION_KIND, COPY_FILE_KIND, COUNT_PRESENTATION_KIND, GATE_KIND, KEYBOARD_KIND,
    LATEST_KIND, LAYOUT_ALIGN_KIND, LAYOUT_COLUMN_KIND, LAYOUT_INSET_KIND, LAYOUT_ROW_KIND,
    LAYOUT_STACK_KIND, LAYOUT_VIEWPORT_KIND, LOGIC_COMPARE_KIND, LOGIC_NOT_KIND, LOGIC_SELECT_KIND,
    MATH_CLAMP_KIND, MATH_DEADBAND_KIND, MATH_SCALE_KIND, ROBOTICS_DRIVE_DIFFERENTIAL_KIND,
    ROBOTICS_OBSERVE_BATTERY_KIND, ROBOTICS_OBSERVE_BUMP_KIND, ROBOTICS_OBSERVE_IMU_KIND,
    ROBOTICS_OBSERVE_ODOMETRY_KIND, ROBOTICS_OBSERVE_RANGE_KIND, ROBOTICS_VELOCITY_INTENT_KIND,
    STATE_COUNT_KIND, STATE_TOGGLE_KIND, TEE_KIND, TEXT_JOIN_KIND, TEXT_LITERAL_KIND,
    TEXT_PRESENTATION_KIND, TEXT_UPPER_KIND, TICK_KIND, TICK_PRESENTATION_KIND, TIME_DEBOUNCE_KIND,
    TIME_DELAY_KIND, TIME_EVERY_KIND, TIME_THROTTLE_KIND, TIME_TIMEOUT_KIND,
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
}

impl PaletteCategory {
    pub const ALL: [Self; 7] = [
        Self::TimeAndFlow,
        Self::Transform,
        Self::State,
        Self::Presentation,
        Self::Files,
        Self::Robotics,
        Self::Input,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteIconKey {
    Clock,
    Repeat2,
    Presentation,
    Type,
    CaseUpper,
    Combine,
    Tally5,
    ChartColumnsIncreasing,
    FileOutput,
    Keyboard,
    GenericGear,
}

impl PaletteIconKey {
    pub const ALL_UPSTREAM: [Self; 10] = [
        Self::Clock,
        Self::Repeat2,
        Self::Presentation,
        Self::Type,
        Self::CaseUpper,
        Self::Combine,
        Self::Tally5,
        Self::ChartColumnsIncreasing,
        Self::FileOutput,
        Self::Keyboard,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Repeat2 => "repeat-2",
            Self::Presentation => "presentation",
            Self::Type => "type",
            Self::CaseUpper => "case-upper",
            Self::Combine => "combine",
            Self::Tally5 => "tally-5",
            Self::ChartColumnsIncreasing => "chart-no-axes-column-increasing",
            Self::FileOutput => "file-output",
            Self::Keyboard => "keyboard",
            Self::GenericGear => "conduit-generic-gear",
        }
    }

    pub const fn accessibility_name(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Repeat2 => "repeating flow",
            Self::Presentation => "presentation screen",
            Self::Type => "text",
            Self::CaseUpper => "uppercase letters",
            Self::Combine => "combined values",
            Self::Tally5 => "count tally",
            Self::ChartColumnsIncreasing => "count chart",
            Self::FileOutput => "file output",
            Self::Keyboard => "keyboard input",
            Self::GenericGear => "generic Gear; icon metadata missing",
        }
    }

    pub const fn is_fallback(self) -> bool {
        matches!(self, Self::GenericGear)
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
        TIME_EVERY_KIND => metadata(
            PaletteCategory::TimeAndFlow,
            &["timer", "interval", "repeat"],
            PaletteIconKey::Repeat2,
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
        TEXT_LITERAL_KIND => metadata(
            PaletteCategory::Transform,
            &["source", "constant", "string"],
            PaletteIconKey::Type,
        ),
        TEXT_UPPER_KIND => metadata(
            PaletteCategory::Transform,
            &["uppercase", "case", "string"],
            PaletteIconKey::CaseUpper,
        ),
        TEXT_JOIN_KIND => metadata(
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
        COPY_FILE_KIND => metadata(
            PaletteCategory::Files,
            &["copy", "filesystem", "resource"],
            PaletteIconKey::FileOutput,
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
        assert_eq!(contracts.len(), 39);
        for contract in contracts {
            let metadata = palette_metadata(&contract.kind_id).expect("palette metadata");
            assert!(!metadata.tags.is_empty());
            assert!(!metadata.icon.is_fallback());
        }
    }

    #[test]
    fn categories_and_upstream_icon_keys_are_finite_and_unique() {
        assert_eq!(PaletteCategory::ALL.len(), 7);
        let keys = PaletteIconKey::ALL_UPSTREAM
            .iter()
            .map(|key| key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), PaletteIconKey::ALL_UPSTREAM.len());
    }
}
