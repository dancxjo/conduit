//! Machine-readable inventory derived from the browser Host's planning offers.

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct InventoryEntry {
    pub kind_id: String,
    pub family: &'static str,
    pub classification: &'static str,
    pub implementation_id: Option<String>,
    pub artifact_id: Option<String>,
    pub reason: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct InventoryDocument {
    pub schema: &'static str,
    pub limits: super::limits::BrowserEnvelopeLimits,
    pub entries: Vec<InventoryEntry>,
}

pub(crate) fn inventory() -> InventoryDocument {
    let advertisement = super::advertisement(
        conduit_core::HostId::from("browser/inventory"),
        conduit_core::BootId::from("browser/inventory-boot"),
    );
    let mut entries = advertisement
        .capabilities
        .into_iter()
        .map(|offer| InventoryEntry {
            family: family(offer.kind_id.as_str()),
            classification: if offer.host_operations.is_empty() {
                "pure-kernel-or-local"
            } else {
                "bounded-browser-host-operation"
            },
            implementation_id: Some(offer.implementation.implementation_id.as_str().into()),
            artifact_id: Some(offer.implementation.artifact_id.as_str().into()),
            kind_id: offer.kind_id.as_str().into(),
            reason: "advertised by the same finite browser Host profile used for planning",
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.kind_id.cmp(&right.kind_id));
    InventoryDocument {
        schema: "conduit.browser/installed-gear-inventory@1",
        limits: super::envelope_limits(),
        entries,
    }
}

fn family(kind: &str) -> &'static str {
    match kind {
        conduit_web::JSON_ENCODE_KIND
        | conduit_web::JSON_DECODE_KIND
        | conduit_web::JSON_COLLECTION_STEP_KIND
        | conduit_web::JSON_BOOLEAN_SUMMARY_KIND => "json",
        conduit_text::TEXT_LITERAL_KIND
        | conduit_text::TEXT_UPPER_KIND
        | conduit_text::TEXT_JOIN_KIND => "text",
        conduit_text::TEXT_MORSE_KIND
        | conduit_text::TEXT_CHARACTERS_KIND
        | conduit_text::MORSE_LOOKUP_KIND
        | conduit_text::MORSE_INTERSPERSE_KIND
        | conduit_text::MORSE_FLATTEN_KIND
        | conduit_text::MORSE_SYMBOLS_TO_PATTERN_KIND
        | conduit_text::MORSE_PATTERN_TO_SYMBOLS_KIND
        | conduit_text::MORSE_SYMBOLS_TO_TEXT_KIND => "morse-composition",
        conduit_language::TOKENIZE_FOUR_KIND | conduit_language::ANNOTATE_FOUR_KIND => {
            "linguistic-structured-info"
        }
        conduit_semantic_catalog::MATH_CLAMP_KIND
        | conduit_semantic_catalog::MATH_SCALE_KIND
        | conduit_semantic_catalog::MATH_DEADBAND_KIND
        | conduit_semantic_catalog::QUANTITY_MAP_KIND => "math",
        conduit_semantic_catalog::LOGIC_COMPARE_KIND
        | conduit_semantic_catalog::LOGIC_NOT_KIND
        | conduit_semantic_catalog::LOGIC_SELECT_KIND => "logic",
        conduit_semantic_catalog::SCALAR_LITERAL_KIND
        | conduit_semantic_catalog::BOOL_LITERAL_KIND => "typed-values",
        conduit_semantic_catalog::TIMED_BUTTON_ATTEMPT_KIND
        | conduit_time::TIME_EVERY_KIND
        | conduit_semantic_catalog::TIME_DELAY_KIND
        | conduit_semantic_catalog::ORDERED_EVENT_INTERVALS_KIND
        | conduit_semantic_catalog::NORMALIZE_SEQUENCE_KIND => "time",
        conduit_semantic_catalog::KEYBOARD_KIND => "interaction",
        conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND => "layout",
        conduit_semantic_catalog::STATE_COUNT_KIND => "state",
        conduit_semantic_catalog::TEXT_PRESENTATION_KIND
        | conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND
        | conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND
        | conduit_semantic_catalog::SCALAR_VALUE_PRESENTATION_KIND
        | conduit_semantic_catalog::BOOL_VALUE_PRESENTATION_KIND
        | conduit_semantic_catalog::COUNT_PRESENTATION_KIND
        | conduit_semantic_catalog::BOOL_PRESENTATION_KIND => "presentation",
        _ => "other-installed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_inventory_is_exactly_derived_from_truthful_planning_offers() {
        let advertisement = super::super::advertisement(
            conduit_core::HostId::from("browser/inventory-test"),
            conduit_core::BootId::from("browser/inventory-test-boot"),
        );
        let inventory = inventory();
        let installed = inventory
            .entries
            .iter()
            .filter_map(|entry| entry.implementation_id.as_deref())
            .collect::<std::collections::BTreeSet<_>>();
        let advertised = advertisement
            .capabilities
            .iter()
            .map(|offer| offer.implementation.implementation_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(installed, advertised);
        for implementation in [
            "browser/kernel-time-every@1",
            "browser/kernel-state-count@1",
            "browser/presentation-count@1",
            "browser/kernel-logic-select-scalar@1",
            "browser/kernel-layout-viewport@1",
            "browser/kernel-time-delay-bool@1",
            "browser/window-keyboard@1",
            "browser/presentation-bool@1",
        ] {
            assert!(installed.contains(implementation));
        }
        assert!(advertisement.resources.iter().any(|resource| {
            resource.pool_id.as_str() == "browser/timer"
                && resource.class_id.as_str() == conduit_core::TIMER_RESOURCE_CLASS
                && resource.capacity_units == 1
        }));
        assert!(
            inventory
                .entries
                .iter()
                .filter(|entry| entry.implementation_id.is_some())
                .map(|entry| entry.family)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= 6
        );
        assert!(inventory
            .entries
            .iter()
            .all(|entry| entry.implementation_id.is_some()));
        assert!(inventory.entries.iter().all(|entry| entry
            .artifact_id
            .as_deref()
            .is_some_and(|identity| !identity.is_empty())));
        assert_eq!(inventory.limits.maximum_gears, 16);
        assert_eq!(inventory.limits.maximum_cords, 24);
        assert_eq!(inventory.limits.maximum_value_bytes, 4_096);
        assert_eq!(inventory.limits.total_value_bytes, 512 * 1_024);
        assert!(advertisement.capabilities.iter().all(|offer| {
            offer.host_operations.len() <= usize::from(inventory.limits.host_operations_per_gear)
        }));
    }
}
