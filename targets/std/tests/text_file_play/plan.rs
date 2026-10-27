use super::*;
use std::collections::BTreeMap;

pub fn reference() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([11; 32]),
        content_profile: kind_id("value/text"),
        access_class: conduit_data::DATA_GENERATION_ACCESS_CLASS.into(),
        extent: ResourceExtent {
            bytes: 11,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([12; 32]),
            expires_at: None,
        },
    }
}

pub fn host(root: &std::path::Path, boot: &str, write: bool) -> conduit_std_host::StdHost {
    conduit_std_host::StdHost::new_with_text_file(
        conduit_std_host::StdHostConfig {
            host_id: "notebook-storage".into(),
            boot_id: boot.into(),
            offer_generation: OfferGeneration(1),
        },
        std::fs::File::open(root).unwrap(),
        reference(),
        write,
    )
    .unwrap()
}

pub fn plan(host: &conduit_std_host::StdHost, write: bool) -> Plan {
    try_plan(host, write, true).unwrap()
}
pub fn try_plan(
    host: &conduit_std_host::StdHost,
    write: bool,
    terminal_output: bool,
) -> Result<Plan, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    startup
        .insert_value_kind_alias("Text", kind_id("value/text"))
        .unwrap();
    conduit_data::install_data_text_catalogs(&mut startup, &mut profile).unwrap();
    let (source, entry, kind) = if write {
        (
            include_str!("../../../../plots/notebook/publish.conduit"),
            "notebook/publish",
            conduit_data::DATA_SAVE_TEXT_KIND,
        )
    } else {
        (
            include_str!("../../../../plots/notebook/load.conduit"),
            "notebook/load",
            conduit_data::DATA_LOAD_TEXT_KIND,
        )
    };
    let source = if terminal_output {
        source.to_owned()
    } else {
        source
            .lines()
            .filter(|line| !line.trim_start().starts_with("failed:") && !line.contains(">> failed"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .map_err(|error| format!("{error:?}"))?;
    let authored = conduit_plot::expand_canonical_plot_for_authoring(&checked, entry, &profile)
        .map_err(|error| format!("{error:?}"))?;
    let advertisement = host.advertisement();
    let offer = advertisement
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == kind)
        .unwrap();
    assert_eq!(offer.authority_requirements.len(), 1);
    assert_eq!(
        advertisement
            .capabilities
            .iter()
            .filter(|offer| [
                conduit_data::DATA_SAVE_TEXT_KIND,
                conduit_data::DATA_LOAD_TEXT_KIND
            ]
            .contains(&offer.kind_id.as_str()))
            .count(),
        1
    );
    let required = &offer.authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: format!("grant/{}/file", advertisement.boot_id.as_str()).into(),
        contract_id: required.contract_id.clone(),
        host_call_contract_id: required.host_call_contract_id.clone(),
        subject_kind: required.subject_kind.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&authored.expanded, &hosts)
        .map_err(|error| format!("{error:?}"))?;
    let mut boundaries = BTreeMap::new();
    for (direction, name, bytes) in if write {
        vec![
            (PortDirection::Input, "value", 4096),
            (PortDirection::Output, "saved", 512),
            (PortDirection::Output, "failed", 1),
        ]
    } else {
        vec![
            (PortDirection::Input, "selected", 512),
            (PortDirection::Output, "value", 4096),
            (PortDirection::Output, "failed", 1),
        ]
    } {
        boundaries.insert(
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: if name == "failed" {
                    ConnectionTrack::AbnormalTerminal
                } else {
                    ConnectionTrack::Payload
                },
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        );
    }
    conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &authored,
        &profile,
        &conduit_plot::CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|error| format!("{error:?}"))
}
