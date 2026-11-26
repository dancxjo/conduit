//! CPU advertisement/Body-admission fixtures; file effects use the real provider.
use super::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_std_host::json_snapshot_file::SelectedJsonSnapshotResidence;
use std::{collections::BTreeMap, fs::File, path::Path};
pub fn selected(files: &Path, boot: &str) -> SelectedJsonSnapshotResidence {
    let reference = catalogue_reference();
    let original = c::AuthorityGrant {
        grant_id: "fixture/catalogue-publish".into(),
        contract_id: conduit_std_offers::JSON_SNAPSHOT_AUTHORITY.into(),
        host_call_contract_id: conduit_std_offers::JSON_SNAPSHOT_PUBLISH_CALL.into(),
        subject_kind: conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND.into(),
        host_id: "host/catalogue-runtime".into(),
        boot_id: boot.into(),
        capability_id: conduit_std_offers::JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION.into(),
    };
    let local = c::ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "fixture/catalogue-file".into(),
        authority_contract: original.contract_id.clone(),
        authority_grant: original.grant_id.clone(),
        maximum_bytes: 4096,
        maximum_items: Some(1),
        availability: c::ResourceReferenceAvailability::Available,
    };
    SelectedJsonSnapshotResidence::prepare(File::open(files).unwrap(), reference, original, local)
        .unwrap()
}
pub fn prepare(
    files: &Path,
    boot: &str,
    write: bool,
) -> (
    conduit_std_host::StdHost,
    c::PlanFragment,
    Option<SelectedJsonSnapshotResidence>,
) {
    let compiled = if write {
        source::checkpoint(&catalogue_reference())
    } else {
        source::read_only()
    }
    .unwrap();
    let config = conduit_std_host::StdHostConfig {
        host_id: "host/catalogue-runtime".into(),
        boot_id: boot.into(),
        offer_generation: c::OfferGeneration(1),
    };
    let mut initial = conduit_std_host::StdHost::new_with_config(config);
    let selected = write.then(|| selected(files, boot));
    if let Some(selected) = &selected {
        selected.install_offer(&mut initial).unwrap();
    }
    let mut ad = initial.advertisement().clone();
    let seed = compiled
        .plot
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == c::STATE_VALUE_KIND)
        .unwrap();
    let c::ConfigurationValue::Structured(initial) = &seed
        .configuration
        .iter()
        .find(|entry| entry.key == "initial")
        .unwrap()
        .value
    else {
        panic!("State initial")
    };
    let initial = c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    ad.capabilities
        .retain(|offer| offer.kind_id.as_str() != c::STATE_VALUE_KIND);
    ad.capabilities.push(
        conduit_std_offers::state_value_durable_std_offer(
            seed.startup_parameters[0].value_type.as_str(),
            &compiled.value_type,
            &initial,
        )
        .unwrap(),
    );
    ad.resources.push(c::resource_offer(
        "fixture/catalogue-state",
        conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS,
        1,
    ));
    for gear in &compiled.plot.expanded.gears {
        if let Some(c::ConfigurationEntry {
            value: c::ConfigurationValue::Text(program),
            ..
        }) = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "program")
        {
            let program =
                conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
            ad.capabilities.push(
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .unwrap(),
            );
        }
    }
    ad.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    ad.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let hosts = [ad.clone()];
    let placements = default_expanded_placements(&compiled.plot.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &compiled.plot.input_bindings),
        (c::PortDirection::Output, &compiled.plot.output_bindings),
    ] {
        for binding in bindings {
            boundaries.insert(
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if direction == c::PortDirection::Input {
                        4096
                    } else {
                        1024
                    },
                },
            );
        }
    }
    let grants = selected
        .as_ref()
        .map(|selected| vec![selected.authority().clone()])
        .unwrap_or_default();
    let empty = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &compiled.plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    (
        conduit_std_host::StdHost::from_advertisement(ad).unwrap(),
        plan.fragments[0].clone(),
        selected,
    )
}
