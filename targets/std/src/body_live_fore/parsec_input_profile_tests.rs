//! Actual constructor offers and authored combined Plan admission, before Play.
use super::input_profile;
use crate::{BodyLiveForeQueue, RunControl, StdHost, StdHostConfig};
use conduit_core::{port_id, ConnectionTrack, HostAdvertisement, Plan, PortDirection};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};
use std::collections::BTreeMap;

pub(super) fn plan(host: &HostAdvertisement, source: &str, key_bytes: u32) -> Result<Plan, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    crate::flow_activation::install_parsec_scan_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)?;
    startup.insert_value_kind_alias(
        "KeyEvent",
        conduit_core::kind_id(conduit_human::KEY_EVENT_INFO_ID),
    )?;
    startup.insert_structured_type(
        "InputButtonTransition",
        conduit_semantic_catalog::input_button_transition_type(),
    )?;
    let source = format!("{}\n{source}", crate::flow_activation::PARSEC_SCAN_SOURCE);
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
        .map_err(|e| format!("combined source: {e:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "parsec/input-main", &profile)
        .map_err(|e| format!("combined expansion: {e:?}"))?;
    let hosts = [host.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded.expanded, &hosts)
        .map_err(|e| format!("combined placement: {e:?}"))?;
    let bounds = [
        (PortDirection::Input, "commands", 4096),
        (PortDirection::Output, "states", 4096),
        (PortDirection::Output, "keys", key_bytes),
        (
            PortDirection::Output,
            "fire",
            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
        ),
    ]
    .into_iter()
    .map(|(direction, name, bytes)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        )
    })
    .collect::<BTreeMap<_, _>>();
    conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &expanded,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &bounds,
    )
    .map_err(|e| format!("combined planning: {e:?}"))
}

fn config() -> StdHostConfig {
    StdHostConfig {
        host_id: "std/parsec-input-profile".into(),
        boot_id: "std/parsec-input-profile/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    }
}

// Explicit software fixture: select source capabilities and resource custody
// before constructing the runner. This does not assert acquired physical input.
fn input_host(config: StdHostConfig) -> StdHost {
    let mut advertisement = crate::composition::build_advertisement(
        config,
        crate::StdHostComposition::reference(),
        None,
        None,
        None,
        false,
    );
    let mut offers = crate::flow_activation::required_parsec_source_offers().unwrap();
    offers.extend([
        crate::flow_activation::parsec_combine_offer().unwrap(),
        crate::flow_activation::parsec_scan_offer(32).unwrap(),
        conduit_std_offers::hosted_keyboard_offer(
            "proof/parsec-input-keyboard",
            "proof/parsec-input-keyboard@1",
        ),
        conduit_std_offers::button::offer(),
    ]);
    for offer in offers {
        if let Some(existing) = advertisement
            .capabilities
            .iter()
            .find(|existing| existing.capability_id == offer.capability_id)
        {
            assert_eq!(existing, &offer);
        } else {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement.resources.push(conduit_core::resource_offer(
        "proof/parsec-input-software",
        conduit_core::INPUT_RESOURCE_CLASS,
        2,
    ));
    advertisement.resources.sort();
    StdHost::from_advertisement(advertisement).unwrap()
}

const SOURCE: &str = include_str!("../flow_activation/parsec_input_scan.conduit");

impl BodyLiveForeQueue {
    pub(crate) fn parsec_input_test_host(config: StdHostConfig) -> StdHost {
        input_host(config)
    }

    pub(crate) fn authored_parsec_input_plan_on_host(host: &HostAdvertisement) -> Plan {
        plan(host, SOURCE, conduit_human::KEY_EVENT_ENCODED_LEN as u32).unwrap()
    }
}

#[test]
fn actual_combined_source_plan_admits_exact_current_three_placement_profile() {
    let host = input_host(config());
    let plan = BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    input_profile::validate(&plan, host.advertisement()).unwrap();
    BodyLiveForeQueue::for_parsec_plan(&plan, RunControl::default(), 1).unwrap();
}

#[test]
fn combined_profile_refuses_different_current_host_boot_and_generation() {
    let host = input_host(config());
    let plan = BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    for config in [
        StdHostConfig {
            host_id: "different/host".into(),
            ..config()
        },
        StdHostConfig {
            boot_id: "different/boot".into(),
            ..config()
        },
        StdHostConfig {
            offer_generation: conduit_core::OfferGeneration(2),
            ..config()
        },
    ] {
        let different = input_host(config);
        assert!(input_profile::validate(&plan, different.advertisement()).is_err());
    }
}

#[test]
fn combined_profile_refuses_authored_extra_source_and_different_key_boundary() {
    let host = input_host(config());
    let extra = SOURCE.replace(
        "keyboard: input/keyboard",
        "keyboard: input/keyboard\n    extra: input/keyboard",
    );
    for candidate in [
        plan(host.advertisement(), &extra, 3),
        plan(host.advertisement(), SOURCE, 4),
    ] {
        assert!(candidate
            .map(|plan| input_profile::validate(&plan, host.advertisement()).is_err())
            .unwrap_or(true));
    }
}
