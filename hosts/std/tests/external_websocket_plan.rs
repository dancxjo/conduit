use conduit_core::{
    BootId, ConnectionProvider, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PlannerCapabilityOffer, PlannerProfileId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../examples/webchat.conduit");

fn browser() -> HostAdvertisement {
    let family = conduit_net::browser_external_websocket_family();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser-chat"),
        boot_id: BootId::from("browser-chat-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser"),
        resources: vec![family.resource],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(conduit_planner::FULL_PLANNER_PROFILE),
            limits: conduit_planner::FULL_PLANNER_LIMITS,
        }],
        capabilities: vec![family.capability],
    }
}

#[test]
fn canonical_transport_forms_plan_to_exact_opt_in_browser_and_std_families() {
    let syntax = parse_syntax_document(SOURCE);
    assert_eq!(syntax.round_trip(), SOURCE);
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "webchat-transport-demo", &profile).unwrap();

    let std = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("std-chat"),
            boot_id: BootId::from("std-chat-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_external_websocket(),
    );
    let realm = [browser(), std.advertisement().clone()];
    let placements = conduit_planner::PlacementChoices {
        by_operation: expanded
            .operations
            .iter()
            .map(|operation| {
                let (host_id, capability_id) =
                    if operation.kind_id.as_str() == conduit_net::EXTERNAL_WEBSOCKET_CLIENT_KIND {
                        (&realm[0].host_id, &realm[0].capabilities[0].capability_id)
                    } else {
                        (&realm[1].host_id, &realm[1].capabilities[0].capability_id)
                    };
                (
                    operation.operation_id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host_id.clone(),
                        capability_id: capability_id.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    };
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        &realm,
        &placements,
        &[ConnectionProvider::Local],
    )
    .unwrap();

    assert_eq!(plan.fragments.len(), 2);
    let client = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == conduit_net::EXTERNAL_WEBSOCKET_CLIENT_KIND)
        .unwrap();
    let listener = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| {
            placement.kind_id.as_str() == conduit_net::EXTERNAL_WEBSOCKET_LISTENER_KIND
        })
        .unwrap();
    assert_eq!(client.host_id.as_str(), "browser-chat");
    assert_eq!(listener.host_id.as_str(), "std-chat");
    assert_eq!(client.inputs, realm[0].capabilities[0].inputs);
    assert_eq!(client.outputs, realm[0].capabilities[0].outputs);
    assert_eq!(listener.inputs, realm[1].capabilities[0].inputs);
    assert_eq!(listener.outputs, realm[1].capabilities[0].outputs);
    assert_ne!(client.operation_id, listener.operation_id);
}
