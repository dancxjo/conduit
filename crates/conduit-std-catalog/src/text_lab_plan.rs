//! Exact two-Host realization fixture for the unchanged canonical Text Lab.

use crate::{
    browser_text_upper_offer, hosted_keyboard_offer, install_input_semantic_catalogs,
    install_keyboard_catalogs, install_text_pipeline_catalogs, standard_host_advertisement,
    KEYBOARD_KIND, KEYMAP_KIND, TEXT_PRESENTATION_KIND,
};
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    process_owned_line_offer_with_limits, resource_offer, BootId, ConnectionBase,
    HostAdvertisement, HostId, LineOffer, LinkLimits, OfferGeneration, Plan, INPUT_RESOURCE_CLASS,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

pub const TEXT_LAB_SPLIT_SOURCE: &str = include_str!("../../../examples/text-lab.conduit");
pub const TEXT_LAB_NATIVE_HOST: &str = "text-lab/native";
pub const TEXT_LAB_NATIVE_BOOT: &str = "text-lab/native/boot-1";
pub const TEXT_LAB_BROWSER_HOST: &str = "text-lab/browser";
pub const TEXT_LAB_BROWSER_BOOT: &str = "text-lab/browser/boot-1";
pub const TEXT_LAB_FORWARD_LINE: &str = "text-lab/native-to-browser";
pub const TEXT_LAB_RETURN_LINE: &str = "text-lab/browser-to-native";
pub const TEXT_LAB_MAXIMUM_VALUES: usize = 5;

/// Bounded machine-readable observation emitted when the live Text Lab loses
/// its planned return Line. This is shared contract data, not std-Host state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextLabLineLossReceipt {
    pub schema: String,
    pub code: String,
    pub phase: String,
    pub sequence: u64,
    pub line_id: String,
    pub plan_id: String,
    pub source_document_id: String,
    pub checked_form_id: String,
    pub active_play_id: String,
    pub sign_id: String,
    pub old_plan_disposition: String,
    pub fresh_planning: String,
    pub form_unchanged: bool,
    pub refusal: String,
    pub transport_failure: String,
}

pub struct TextLabSplitPlan {
    pub plan: Plan,
    pub native: HostAdvertisement,
    pub browser: HostAdvertisement,
    pub forward_line: LineOffer,
    pub return_line: LineOffer,
}

pub struct TextLabLineLossOutcome {
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub immutable_plan_id: conduit_core::PlanId,
    pub unavailable_line_id: conduit_core::LineId,
    pub refusal: String,
}

pub fn exact_text_lab_split_plan(base_instance: &str) -> Result<TextLabSplitPlan, String> {
    exact_text_lab_split_plan_with_loss(base_instance, None)
}

pub fn exact_text_lab_line_loss_outcome(
    base_instance: &str,
    unavailable_line: &str,
) -> Result<TextLabLineLossOutcome, String> {
    if !matches!(
        unavailable_line,
        TEXT_LAB_FORWARD_LINE | TEXT_LAB_RETURN_LINE
    ) {
        return Err("unknown Text Lab Line loss target".into());
    }
    let accepted = exact_text_lab_split_plan_with_loss(base_instance, None)?;
    let immutable_plan_id = accepted.plan.plan_id.clone();
    let refusal = match exact_text_lab_split_plan_with_loss(base_instance, Some(unavailable_line)) {
        Ok(_) => return Err("lost selected Text Lab Line still produced a Plan".into()),
        Err(refusal) => refusal,
    };
    if accepted.plan.plan_id != immutable_plan_id || !conduit_core::verify_plan(&accepted.plan) {
        return Err("Text Lab Line loss mutated the accepted Plan".into());
    }
    Ok(TextLabLineLossOutcome {
        source_document_id: accepted.plan.source_document_id,
        checked_form_id: accepted.plan.checked_form_id,
        immutable_plan_id,
        unavailable_line_id: conduit_core::LineId::from(unavailable_line),
        refusal,
    })
}

fn exact_text_lab_split_plan_with_loss(
    base_instance: &str,
    unavailable_line: Option<&str>,
) -> Result<TextLabSplitPlan, String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_keyboard_catalogs(&mut startup, &mut profile)?;
    install_input_semantic_catalogs(&mut startup, &mut profile)?;
    install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    let checked = conduit_form::check_syntax_document(
        &conduit_form::parse_syntax_document(TEXT_LAB_SPLIT_SOURCE),
        &startup,
    )
    .map_err(|error| format!("check canonical Text Lab: {error:?}"))?;
    let expanded = conduit_form::expand_canonical_form(&checked, "text-lab", &profile)
        .map_err(|error| format!("expand canonical Text Lab: {error:?}"))?;
    let mut native = standard_host_advertisement(
        HostId::from(TEXT_LAB_NATIVE_HOST),
        BootId::from(TEXT_LAB_NATIVE_BOOT),
        OfferGeneration(1),
    );
    native.capabilities.push(hosted_keyboard_offer(
        "text-lab-native-keyboard",
        "text-lab/native-keyboard@1",
    ));
    native.resources.push(resource_offer(
        "text-lab/native-input",
        INPUT_RESOURCE_CLASS,
        1,
    ));
    native
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    native
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let browser = HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: HostId::from(TEXT_LAB_BROWSER_HOST),
        boot_id: BootId::from(TEXT_LAB_BROWSER_BOOT),
        offer_generation: OfferGeneration(1),
        profile: conduit_core::HostProfileId::from("browser/text-lab@1"),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities: vec![browser_text_upper_offer()],
    };
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_buffered_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_frame_bytes: 1_024,
    };
    let mut forward_line = process_owned_line_offer_with_limits(
        TEXT_LAB_FORWARD_LINE,
        "text-lab/native-to-browser-binding",
        ConnectionBase::WebSocket,
        base_instance,
        &native,
        &browser,
        limits,
    );
    let mut return_line = process_owned_line_offer_with_limits(
        TEXT_LAB_RETURN_LINE,
        "text-lab/browser-to-native-binding",
        ConnectionBase::WebSocket,
        base_instance,
        &browser,
        &native,
        limits,
    );
    match unavailable_line {
        Some(TEXT_LAB_FORWARD_LINE) => {
            forward_line.availability.availability = conduit_core::LineAvailability::Unavailable;
        }
        Some(TEXT_LAB_RETURN_LINE) => {
            return_line.availability.availability = conduit_core::LineAvailability::Unavailable;
        }
        Some(_) => return Err("unknown Text Lab Line loss target".into()),
        None => {}
    }
    let capability = |host: &HostAdvertisement, kind: &str| {
        host.capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == kind)
            .ok_or_else(|| format!("Text Lab Host lacks {kind}"))
            .map(|offer| offer.capability_id.clone())
    };
    let by_gear = BTreeMap::from([
        (
            conduit_core::GearId::from("text-lab/keyboard"),
            PlacementChoice {
                host_id: native.host_id.clone(),
                capability_id: capability(&native, KEYBOARD_KIND)?,
            },
        ),
        (
            conduit_core::GearId::from("text-lab/keymap"),
            PlacementChoice {
                host_id: native.host_id.clone(),
                capability_id: capability(&native, KEYMAP_KIND)?,
            },
        ),
        (
            conduit_core::GearId::from("text-lab/uppercase"),
            PlacementChoice {
                host_id: browser.host_id.clone(),
                capability_id: capability(&browser, conduit_text::TEXT_UPPER_KIND)?,
            },
        ),
        (
            conduit_core::GearId::from("text-lab/presentation"),
            PlacementChoice {
                host_id: native.host_id.clone(),
                capability_id: capability(&native, TEXT_PRESENTATION_KIND)?,
            },
        ),
    ]);
    let line_candidates = BTreeMap::from([
        (
            (
                conduit_core::GearId::from("text-lab/keymap"),
                conduit_core::GearId::from("text-lab/uppercase"),
            ),
            vec![forward_line.line_id.clone()],
        ),
        (
            (
                conduit_core::GearId::from("text-lab/uppercase"),
                conduit_core::GearId::from("text-lab/presentation"),
            ),
            vec![return_line.line_id.clone()],
        ),
    ]);
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &[native.clone(), browser.clone()],
        &PlacementChoices { by_gear },
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 24,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[forward_line.clone(), return_line.clone()],
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(TextLabSplitPlan {
        plan,
        native,
        browser,
        forward_line,
        return_line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_profile_keeps_source_clean_and_seals_two_directional_lines() {
        for forbidden in ["browser", "websocket", "host", "line", "address"] {
            assert!(!TEXT_LAB_SPLIT_SOURCE
                .to_ascii_lowercase()
                .contains(forbidden));
        }
        let exact = exact_text_lab_split_plan("ws://127.0.0.1:1/conduit").unwrap();
        assert!(conduit_core::verify_plan(&exact.plan));
        assert_eq!(exact.plan.fragments.len(), 2);
        assert_ne!(exact.forward_line.line_id, exact.return_line.line_id);
        assert_eq!(
            exact.forward_line.binding.source.host_id,
            exact.native.host_id
        );
        assert_eq!(
            exact.return_line.binding.source.host_id,
            exact.browser.host_id
        );
    }

    #[test]
    fn either_selected_line_loss_preserves_the_old_plan_and_refuses_fresh_planning() {
        let base = "ws://127.0.0.1:1/conduit";
        let accepted = exact_text_lab_split_plan(base).unwrap();
        for line in [TEXT_LAB_FORWARD_LINE, TEXT_LAB_RETURN_LINE] {
            let loss = exact_text_lab_line_loss_outcome(base, line).unwrap();
            assert_eq!(loss.source_document_id, accepted.plan.source_document_id);
            assert_eq!(loss.checked_form_id, accepted.plan.checked_form_id);
            assert_eq!(loss.immutable_plan_id, accepted.plan.plan_id);
            assert_eq!(loss.unavailable_line_id.as_str(), line);
            assert!(loss.refusal.contains("unavailable"));
        }
    }
}
