//! Canonical discovery metadata for the supported user-facing Kind nucleus.
//!
//! Categories, tags, and icons help people find a Kind. They are deliberately
//! absent from Kind contracts and therefore cannot alter semantic identity.

use conduit_core::KindId;

use crate::{
    COPY_FILE_KIND, COUNT_PRESENTATION_KIND, GATE_KIND, LATEST_KIND, STATE_COUNT_KIND, TEE_KIND,
    TEXT_JOIN_KIND, TEXT_LITERAL_KIND, TEXT_PRESENTATION_KIND, TEXT_UPPER_KIND, TICK_KIND,
    TICK_PRESENTATION_KIND, TIME_EVERY_KIND,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaletteCategory {
    TimeAndFlow,
    Transform,
    State,
    Presentation,
    Files,
}

impl PaletteCategory {
    pub const ALL: [Self; 5] = [
        Self::TimeAndFlow,
        Self::Transform,
        Self::State,
        Self::Presentation,
        Self::Files,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::TimeAndFlow => "Time & Flow",
            Self::Transform => "Transform",
            Self::State => "State",
            Self::Presentation => "Presentation",
            Self::Files => "Files",
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
    GenericGear,
}

impl PaletteIconKey {
    pub const ALL_UPSTREAM: [Self; 9] = [
        Self::Clock,
        Self::Repeat2,
        Self::Presentation,
        Self::Type,
        Self::CaseUpper,
        Self::Combine,
        Self::Tally5,
        Self::ChartColumnsIncreasing,
        Self::FileOutput,
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
        TICK_PRESENTATION_KIND => metadata(
            PaletteCategory::Presentation,
            &["display", "tick", "indicator"],
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
        let contracts = crate::supported_nucleus_contracts();
        assert_eq!(contracts.len(), 13);
        for contract in contracts {
            let metadata = palette_metadata(&contract.kind_id).expect("palette metadata");
            assert!(!metadata.tags.is_empty());
            assert!(!metadata.icon.is_fallback());
        }
    }

    #[test]
    fn categories_and_upstream_icon_keys_are_finite_and_unique() {
        assert_eq!(PaletteCategory::ALL.len(), 5);
        let keys = PaletteIconKey::ALL_UPSTREAM
            .iter()
            .map(|key| key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), PaletteIconKey::ALL_UPSTREAM.len());
    }
}
