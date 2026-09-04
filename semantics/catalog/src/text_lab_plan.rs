//! Exact two-Host realization fixture for the unchanged canonical Text Lab.

use crate::{
    install_input_semantic_catalogs, install_keyboard_catalogs, install_text_pipeline_catalogs,
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
    kind_id, present_host_operation_requirement, process_owned_line_offer_with_limits,
    resource_offer, resource_requirement, BaseImplementationId, BootId, CapabilityOffer,
    HostAdvertisement, HostId, HostProfileId, LineOffer, LineScope, LineSecurity, LinkLimits,
    OfferGeneration, Plan, INPUT_RESOURCE_CLASS, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

pub const TEXT_LAB_SPLIT_SOURCE: &str = include_str!("../../../forms/text-lab/main.conduit");
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

pub fn exact_text_lab_split_plan(
    base_instance: &str,
    browser_text_upper: &conduit_core::CapabilityOffer,
) -> Result<TextLabSplitPlan, String> {
    exact_text_lab_split_plan_with_loss(base_instance, browser_text_upper, None)
}

pub fn exact_text_lab_line_loss_outcome(
    base_instance: &str,
    browser_text_upper: &conduit_core::CapabilityOffer,
    unavailable_line: &str,
) -> Result<TextLabLineLossOutcome, String> {
    if !matches!(
        unavailable_line,
        TEXT_LAB_FORWARD_LINE | TEXT_LAB_RETURN_LINE
    ) {
        return Err("unknown Text Lab Line loss target".into());
    }
    let accepted = exact_text_lab_split_plan_with_loss(base_instance, browser_text_upper, None)?;
    let immutable_plan_id = accepted.plan.plan_id.clone();
    let refusal = match exact_text_lab_split_plan_with_loss(
        base_instance,
        browser_text_upper,
        Some(unavailable_line),
    ) {
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
    browser_text_upper: &conduit_core::CapabilityOffer,
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
    let mut native = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(TEXT_LAB_NATIVE_HOST),
        boot_id: BootId::from(TEXT_LAB_NATIVE_BOOT),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("text-lab/native-fixture@1"),
        resources: vec![
            resource_offer("text-lab/native-input", INPUT_RESOURCE_CLASS, 1),
            resource_offer(
                "text-lab/native-presentation",
                PRESENTATION_RESOURCE_CLASS,
                1,
            ),
        ],
        planner_capabilities: Vec::new(),
        capabilities: vec![
            keyboard_fixture_offer(),
            keymap_fixture_offer(),
            text_upper_fixture_offer(
                "text-lab-native-upper",
                "text-lab/native-fixture@1",
                "text-lab/native-upper@1",
                "text-lab/native-fixture@1",
            ),
            text_presentation_fixture_offer(),
        ],
    };
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
        capabilities: vec![browser_text_upper.clone()],
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
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        base_instance,
        &native,
        &browser,
        limits,
    );
    let mut return_line = process_owned_line_offer_with_limits(
        TEXT_LAB_RETURN_LINE,
        "text-lab/browser-to-native-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        base_instance,
        &browser,
        &native,
        limits,
    );
    for line in [&mut forward_line, &mut return_line] {
        line.contract.scope = LineScope::LocalNetwork;
        line.contract.security = LineSecurity::PlaintextNetwork;
    }
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
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
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

fn keyboard_fixture_offer() -> CapabilityOffer {
    let contract = crate::keyboard_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: "text-lab-native-keyboard".into(),
        kind_id: contract.kind_id,
        kind_contract_revision: crate::keyboard_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: "text-lab/native-fixture@1".into(),
            implementation_id: "text-lab/native-keyboard@1".into(),
            artifact_id: "text-lab/native-keyboard@1".into(),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: "proof/input-next-key-event@1".into(),
            target_kind: Some(kind_id(conduit_human::KEY_EVENT_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        }],
        resource_requirements: vec![resource_requirement(INPUT_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn text_presentation_fixture_offer() -> CapabilityOffer {
    crate::realization_offer(
        crate::text_presentation_contract(),
        crate::TEXT_PRESENTATION_CONTRACT_REVISION,
        crate::RealizationOfferIdentity {
            capability: "text-lab/native-text-presentation",
            execution_profile: "text-lab/native-fixture@1",
            implementation: "text-lab/native-text-presentation@1",
            artifact: "text-lab/native-fixture@1",
        },
        vec![present_host_operation_requirement(
            kind_id("presentation/text-lab-native"),
            conduit_text::MAX_TEXT_BYTES,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn keymap_fixture_offer() -> CapabilityOffer {
    crate::realization_offer(
        crate::keymap_contract(),
        crate::KEYMAP_REVISION,
        crate::RealizationOfferIdentity {
            capability: "text-lab/native-keymap",
            execution_profile: "text-lab/native-fixture@1",
            implementation: "text-lab/native-keymap@1",
            artifact: "text-lab/native-fixture@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from("conduit.host/input-keymap@1"),
            target_kind: Some(kind_id("input/keymap-text-fragment")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
            maximum_output_bytes: 4,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn text_upper_fixture_offer(
    capability: &str,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    let mut offer = crate::realization_offer(
        crate::text_upper_contract(),
        conduit_text::TEXT_UPPER_CONTRACT_REVISION,
        crate::RealizationOfferIdentity {
            capability,
            execution_profile,
            implementation,
            artifact,
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from("conduit.host/text-upper@1"),
            target_kind: Some(kind_id("text/uppercase-utf8")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
            maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
        }],
        Vec::new(),
        Vec::new(),
    );
    offer.shorthand = Some((conduit_core::port_id("text"), conduit_core::port_id("text")));
    offer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser_text_upper_fixture() -> conduit_core::CapabilityOffer {
        text_upper_fixture_offer(
            "test-browser-text-upper-v1",
            "test/browser-text-upper@1",
            "test/browser-text-upper@1",
            "test/browser-text-upper@1",
        )
    }

    #[test]
    fn split_profile_keeps_source_clean_and_seals_two_directional_lines() {
        for forbidden in ["browser", "websocket", "host", "line", "address"] {
            assert!(!TEXT_LAB_SPLIT_SOURCE
                .to_ascii_lowercase()
                .contains(forbidden));
        }
        let exact =
            exact_text_lab_split_plan("ws://127.0.0.1:1/conduit", &browser_text_upper_fixture())
                .unwrap();
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
        let browser_upper = browser_text_upper_fixture();
        let accepted = exact_text_lab_split_plan(base, &browser_upper).unwrap();
        for line in [TEXT_LAB_FORWARD_LINE, TEXT_LAB_RETURN_LINE] {
            let loss = exact_text_lab_line_loss_outcome(base, &browser_upper, line).unwrap();
            assert_eq!(loss.source_document_id, accepted.plan.source_document_id);
            assert_eq!(loss.checked_form_id, accepted.plan.checked_form_id);
            assert_eq!(loss.immutable_plan_id, accepted.plan.plan_id);
            assert_eq!(loss.unavailable_line_id.as_str(), line);
            assert!(loss.refusal.contains("unavailable"));
        }
    }
}
