use super::*;
use conduit_core::{port_id, BaseImplementationId, HostAdvertisement, PortDirection};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_activations, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};

use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../../plots/thermostat/main.conduit");

fn authored_with_maximum(
    maximum_items: u16,
) -> (
    conduit_plot::CheckedSyntaxDocument,
    conduit_plot::ExpandedAuthoringPlot,
    ProfileCatalog,
) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_thermostat_plot::install_catalogs(&mut startup, &mut profile).unwrap();
    let source = SOURCE.replace(
        "maximum-items = 256",
        &format!("maximum-items = {maximum_items}"),
    );
    let document = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&document, "thermostat/main", &profile).unwrap();
    (document, authoring, profile)
}

pub(crate) fn authored_thermostat_plan_on_host(
    advertisement: &HostAdvertisement,
    maximum_items: u16,
) -> conduit_core::Plan {
    let (document, authoring, profile) = authored_with_maximum(maximum_items);
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 32,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 3,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 13,
            },
        ),
    ]);
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &bases,
        options,
        &bounds,
    )
    .expect("exact production Todo scan must plan")
}

#[test]
fn thermostat_scan_has_a_finite_domain_bound() {
    let initial = ThermostatState::default();
    assert!(thermostat_scan_offer(&initial, 256).is_ok());
    assert!(thermostat_scan_offer(&initial, 0).is_err());
    assert!(thermostat_scan_offer(&initial, 257).is_err());
}
