//! Canonical Text Lab planning against the browser-owned uppercase realization.

use std::collections::BTreeMap;

use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, BootId, HostId, LineAvailability,
    LinkLimits, OfferGeneration,
};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};

use super::offers::text_advertisement;

const SOURCE: &str = include_str!("../../../../../examples/text-lab.conduit");
const LOCAL_HOST: &str = "text-lab/native";

fn checked_form() -> conduit_form::CheckedForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    conduit_form::parse(SOURCE, &profile).expect("checked-in Text Lab checks unchanged")
}

fn hosts() -> (
    conduit_core::HostAdvertisement,
    conduit_core::HostAdvertisement,
) {
    let mut local = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: HostId::from(LOCAL_HOST),
        boot_id: BootId::from("text-lab/native/boot-1"),
        offer_generation: OfferGeneration(1),
        profile: conduit_core::HostProfileId::from("browser-test/native-text-lab@1"),
        resources: vec![
            conduit_core::resource_offer(
                "text-lab/native-input",
                conduit_core::INPUT_RESOURCE_CLASS,
                1,
            ),
            conduit_core::resource_offer(
                "text-lab/native-presentation",
                conduit_core::PRESENTATION_RESOURCE_CLASS,
                1,
            ),
        ],
        planner_capabilities: Vec::new(),
        capabilities: vec![
            keyboard_fixture_offer(),
            native_keymap_fixture_offer(),
            super::browser_text_upper_offer(),
            super::offer_composition::text_offer(),
        ],
    };
    local
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    local
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    (local, text_advertisement())
}

fn keyboard_fixture_offer() -> conduit_core::CapabilityOffer {
    let contract = conduit_semantic_catalog::keyboard_contract();
    conduit_core::CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: "text-lab-native-keyboard".into(),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_semantic_catalog::keyboard_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: "browser-test/native-text-lab@1".into(),
            implementation_id: "text-lab/native-keyboard@1".into(),
            artifact_id: "text-lab/native-keyboard@1".into(),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: "proof/input-next-key-event@1".into(),
            target_kind: Some(conduit_core::kind_id(conduit_core::KEY_EVENT_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: conduit_core::KEY_EVENT_ENCODED_LEN as u32,
        }],
        resource_requirements: vec![conduit_core::resource_requirement(
            conduit_core::INPUT_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn native_keymap_fixture_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::keymap_contract(),
        conduit_semantic_catalog::KEYMAP_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: "text-lab-native-keymap",
            execution_profile: "text-lab/native-fixture@1",
            implementation: "text-lab/native-keymap@1",
            artifact: "text-lab/native-fixture@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from("conduit.host/input-keymap@1"),
            target_kind: Some(conduit_core::kind_id("input/keymap-text-fragment")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::KEY_EVENT_ENCODED_LEN as u32,
            maximum_output_bytes: 4,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn lines(
    local: &conduit_core::HostAdvertisement,
    browser: &conduit_core::HostAdvertisement,
) -> Vec<conduit_core::LineOffer> {
    let limits = LinkLimits {
        maximum_in_flight_items: 4,
        maximum_payload_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_buffered_bytes: conduit_text::MAX_TEXT_BYTES * 4,
        maximum_frame_bytes: conduit_text::MAX_TEXT_BYTES * 2,
    };
    vec![
        process_owned_line_offer_with_limits(
            "text-lab/native-to-browser",
            "text-lab/native-to-browser-binding",
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "text-lab/native-to-browser-instance",
            local,
            browser,
            limits,
        ),
        process_owned_line_offer_with_limits(
            "text-lab/browser-to-native",
            "text-lab/browser-to-native-binding",
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "text-lab/browser-to-native-instance",
            browser,
            local,
            limits,
        ),
    ]
}

fn split_plan(
    form: &conduit_form::CheckedForm,
    local: &conduit_core::HostAdvertisement,
    browser: &conduit_core::HostAdvertisement,
    lines: &[conduit_core::LineOffer],
) -> Result<conduit_core::Plan, conduit_planner::PlannerError> {
    let capability = |host: &conduit_core::HostAdvertisement, kind: &str| {
        host.capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == kind)
            .expect("required Text Lab offer")
            .capability_id
            .clone()
    };
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                conduit_core::GearId::from("text-lab/keyboard"),
                PlacementChoice {
                    host_id: local.host_id.clone(),
                    capability_id: capability(local, conduit_semantic_catalog::KEYBOARD_KIND),
                },
            ),
            (
                conduit_core::GearId::from("text-lab/keymap"),
                PlacementChoice {
                    host_id: local.host_id.clone(),
                    capability_id: capability(local, conduit_semantic_catalog::KEYMAP_KIND),
                },
            ),
            (
                conduit_core::GearId::from("text-lab/uppercase"),
                PlacementChoice {
                    host_id: browser.host_id.clone(),
                    capability_id: capability(browser, conduit_text::TEXT_UPPER_KIND),
                },
            ),
            (
                conduit_core::GearId::from("text-lab/presentation"),
                PlacementChoice {
                    host_id: local.host_id.clone(),
                    capability_id: capability(
                        local,
                        conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
                    ),
                },
            ),
        ]),
    };
    plan_with_line_offers(
        form,
        &[local.clone(), browser.clone()],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        4,
        24,
        lines,
    )
}

#[test]
fn unchanged_text_lab_selects_browser_uppercase_and_loss_cannot_mutate_its_plan() {
    let form = checked_form();
    let source_document_id = form.source_document_id.clone();
    let checked_form_id = form.checked_form_id.clone();
    let (local, browser) = hosts();
    let mut lines = lines(&local, &browser);
    let plan = split_plan(&form, &local, &browser, &lines).expect("split Text Lab plans");

    assert!(conduit_core::verify_plan(&plan));
    assert_eq!(plan.source_document_id, source_document_id);
    assert_eq!(plan.checked_form_id, checked_form_id);
    let uppercase = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.gear_id.as_str() == "text-lab/uppercase")
        .expect("uppercase placement");
    assert_eq!(uppercase.host_id, browser.host_id);
    assert_eq!(
        uppercase.artifact_id.as_str(),
        super::BROWSER_TEXT_UPPER_ARTIFACT
    );
    for local_gear in [
        "text-lab/keyboard",
        "text-lab/keymap",
        "text-lab/presentation",
    ] {
        assert!(plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .any(|placement| placement.gear_id.as_str() == local_gear
                && placement.host_id == local.host_id));
    }
    assert_eq!(
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .filter(|connection| !connection.admitted_lines.is_empty())
            .map(|connection| &connection.connection_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );

    let immutable_plan_id = plan.plan_id.clone();
    lines[0].availability.availability = LineAvailability::Unavailable;
    let refusal = split_plan(&form, &local, &browser, &lines)
        .expect_err("lost selected Line refuses a replacement Plan");
    assert!(matches!(
        refusal,
        conduit_planner::PlannerError::LineOfferUnavailable(_)
    ));
    assert_eq!(plan.plan_id, immutable_plan_id);
    assert!(conduit_core::verify_plan(&plan));
}
