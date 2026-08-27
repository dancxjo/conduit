//! Canonical Text Lab planning against the browser-owned uppercase realization.

use std::collections::BTreeMap;

use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, BootId, HostId, LineAvailability,
    LinkLimits, OfferGeneration,
};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};

use super::offers::text_advertisement;

const SOURCE: &str = include_str!("../../../../examples/text-lab.conduit");
const LOCAL_HOST: &str = "text-lab/native";

fn checked_form() -> conduit_form::CheckedForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
    conduit_std_catalog::install_input_semantic_catalogs(&mut startup, &mut profile).unwrap();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    conduit_form::parse(SOURCE, &profile).expect("checked-in Text Lab checks unchanged")
}

fn hosts() -> (
    conduit_core::HostAdvertisement,
    conduit_core::HostAdvertisement,
) {
    let mut local = conduit_std_catalog::standard_host_advertisement(
        HostId::from(LOCAL_HOST),
        BootId::from("text-lab/native/boot-1"),
        OfferGeneration(1),
    );
    local
        .capabilities
        .push(conduit_std_catalog::hosted_keyboard_offer(
            "text-lab-native-keyboard",
            "text-lab/native-keyboard@1",
        ));
    local.resources.push(conduit_core::resource_offer(
        "text-lab/native-input",
        conduit_core::INPUT_RESOURCE_CLASS,
        1,
    ));
    local
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    local
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    (local, text_advertisement())
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
                    capability_id: capability(local, conduit_std_catalog::KEYBOARD_KIND),
                },
            ),
            (
                conduit_core::GearId::from("text-lab/keymap"),
                PlacementChoice {
                    host_id: local.host_id.clone(),
                    capability_id: capability(local, conduit_std_catalog::KEYMAP_KIND),
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
                    capability_id: capability(local, conduit_std_catalog::TEXT_PRESENTATION_KIND),
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
        conduit_std_catalog::BROWSER_TEXT_UPPER_ARTIFACT
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
