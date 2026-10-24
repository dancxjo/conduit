//! Actual durable State to catalogue discovery. Body/Host admission and storage
//! grants are fixtures; both State and snapshot file effects are real.
#![cfg(target_os = "linux")]
use conduit_core as c;
use conduit_environment_make::RecordingCatalogSeedSource;
use conduit_std_host as host;
use std::collections::BTreeMap;

fn planned(write: bool, boot: &str) -> (host::StdHost, c::Plan, c::StructuredInfoType) {
    let source = if write {
        RecordingCatalogSeedSource::compile()
    } else {
        RecordingCatalogSeedSource::compile_read_only()
    }
    .unwrap();
    let gear = &source.plot.expanded.gears[0];
    let c::ConfigurationValue::Structured(initial) = &gear
        .configuration
        .iter()
        .find(|entry| entry.key == "initial")
        .unwrap()
        .value
    else {
        panic!("initial")
    };
    let initial = c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    let mut advertisement = host::StdHost::new_with_config(host::StdHostConfig {
        host_id: "fixture/seed-host".into(),
        boot_id: boot.into(),
        offer_generation: c::OfferGeneration(1),
    })
    .advertisement()
    .clone();
    advertisement
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != c::STATE_VALUE_KIND);
    advertisement.capabilities.push(
        conduit_std_offers::state_value_durable_std_offer(
            gear.startup_parameters[0].value_type.as_str(),
            &source.value_type,
            &initial,
        )
        .unwrap(),
    );
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement.resources.push(c::ResourceOffer {
        pool_id: "fixture/seed-state".into(),
        class_id: conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS.into(),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let hosts = [advertisement.clone()];
    let choices =
        conduit_planner::default_expanded_placements(&source.plot.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &source.plot.input_bindings),
        (c::PortDirection::Output, &source.plot.output_bindings),
    ] {
        for binding in bindings {
            boundaries.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 1024,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &source.plot,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (
        host::StdHost::from_advertisement(advertisement).unwrap(),
        plan,
        source.value_type,
    )
}
struct WriteCurrent {
    expected: Vec<u8>,
    control: host::RunControl,
}
impl host::ExternalForeOutputAdapter for WriteCurrent {
    fn deliver(&mut self, value: host::ExternalForeDelivery) -> Result<(), String> {
        if value.bytes.as_slice() == self.expected {
            self.control
                .request_stop(host::RunControlRequestId::new("seed-written").unwrap())
                .map_err(|error| format!("stop: {error:?}"))?;
        }
        Ok(())
    }
}
struct NoWait;
impl host::TimerAdapter for NoWait {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("seed recovery needs no timer wait")
    }
}
fn reference() -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([11; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 1024,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([12; 32]),
            expires_at: None,
        },
    }
}
#[test]
fn installed_seed_restart_recovers_the_actual_current_reference_and_refuses_corruption() {
    let state = tempfile::tempdir().unwrap();
    let (catalog_files, catalog_reference) = publish_catalog();
    let body: conduit_body::BodyId = serde_json::from_str("\"fixture/restart-body\"").unwrap();
    let (mut read, plan, _) = planned(false, "fixture/empty-boot");
    assert_eq!(
        conduit_environment_linux::recover_durable_catalog_seed(
            &mut read,
            &body,
            state.path(),
            plan.fragments[0].clone(),
            &mut NoWait,
        )
        .unwrap(),
        None
    );
    let (mut write, plan, value_type) = planned(true, "fixture/write-boot");
    let selected = c::StructuredInfoValue::variant(
        value_type,
        "some",
        c::StructuredInfoValue::leaf(
            c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).unwrap(),
            catalog_reference.encode().unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let control = host::RunControl::default();
    let mut collector = WriteCurrent {
        expected: selected.clone(),
        control: control.clone(),
    };
    write
        .run_body_durable_external_plot_to(
            &body,
            state.path(),
            plan.fragments[0].clone(),
            host::ExternalForeRun {
                inputs: &[host::ExternalForeInput {
                    front_port_id: "committed".into(),
                    track: c::ConnectionTrack::Payload,
                    bytes: selected,
                }],
                output: &mut collector,
                sequential: false,
                stream: None,
            },
            &mut std::io::sink(),
            &mut NoWait,
            &control,
        )
        .unwrap();
    drop(write);
    let files: Vec<_> = std::fs::read_dir(state.path())
        .unwrap()
        .map(|entry| entry.unwrap().path().join("current"))
        .filter(|path| path.is_file())
        .collect();
    assert_eq!(files.len(), 1);
    let bytes = std::fs::read(&files[0]).unwrap();
    let (mut read, plan, _) = planned(false, "fixture/restart-boot");
    let recovered = conduit_environment_linux::recover_durable_catalog_seed(
        &mut read,
        &body,
        state.path(),
        plan.fragments[0].clone(),
        &mut NoWait,
    )
    .unwrap();
    assert_eq!(recovered, Some(catalog_reference));
    assert_eq!(std::fs::read(&files[0]).unwrap(), bytes);
    inspect_catalog(&catalog_files, recovered.as_ref().unwrap());
    let other: conduit_body::BodyId = serde_json::from_str("\"fixture/other-body\"").unwrap();
    let (mut other_host, other_plan, _) = planned(false, "fixture/other-body-boot");
    assert_eq!(
        conduit_environment_linux::recover_durable_catalog_seed(
            &mut other_host,
            &other,
            state.path(),
            other_plan.fragments[0].clone(),
            &mut NoWait,
        )
        .unwrap(),
        None
    );
    assert_eq!(std::fs::read(&files[0]).unwrap(), bytes);
    std::fs::write(&files[0], &bytes[..bytes.len() - 1]).unwrap();
    let (mut read, plan, _) = planned(false, "fixture/corrupt-boot");
    assert!(conduit_environment_linux::recover_durable_catalog_seed(
        &mut read,
        &body,
        state.path(),
        plan.fragments[0].clone(),
        &mut NoWait,
    )
    .is_err());
    assert_eq!(std::fs::read(&files[0]).unwrap(), bytes[..bytes.len() - 1]);
}

use conduit_resource_snapshot::{PUBLISH_OPERATION, READ_OPERATION};
#[path = "../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
mod snapshot_fixture;
fn catalog_placement(reference: &c::BoundedResourceRef, write: bool) -> c::PlannedGear {
    let (mut placement, _) = snapshot_fixture::placement(
        if write {
            "fixture/catalog/old-boot"
        } else {
            "fixture/catalog/restarted-boot"
        },
        write,
        reference.extent.bytes as usize,
    );
    let resource = placement.resources[0].content.as_mut().unwrap();
    resource.contract.identity = reference.identity;
    resource.contract.version = reference.lifetime.version;
    placement
}
fn publish_catalog() -> (tempfile::TempDir, c::BoundedResourceRef) {
    let content = conduit_environment_runtime::recording::PreparedRecordingCatalog::prepare(
        &[reference()],
        2,
        8192,
    )
    .unwrap();
    let mut selected = reference();
    selected.identity = c::ResourceSemanticIdentity::from_digest([13; 32]);
    selected.extent.bytes = content.content().len() as u64;
    let root = tempfile::tempdir().unwrap();
    let directory = conduit_linux_snapshot_store::SnapshotDirectory::prepare(
        std::fs::File::open(root.path()).unwrap(),
    )
    .unwrap();
    let write = catalog_placement(&selected, true);
    conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(&directory, &write, &selected)
        .unwrap()
        .publish(&write.authority[0], content.content().as_bytes())
        .unwrap();
    (root, selected)
}
fn inspect_catalog(root: &tempfile::TempDir, selected: &c::BoundedResourceRef) {
    let directory = conduit_linux_snapshot_store::SnapshotDirectory::prepare(
        std::fs::File::open(root.path()).unwrap(),
    )
    .unwrap();
    let read = catalog_placement(selected, false);
    let mut recovery = conduit_environment_linux::PreparedFileCatalogRecovery::prepare(
        &directory, &read, selected,
    )
    .unwrap();
    let catalog = recovery.read(&read.authority[0]).unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(
        c::BoundedResourceRef::decode(catalog.encoded_reference(0).unwrap()).unwrap(),
        reference()
    );
}

#[test]
fn installed_seed_recovery_rejects_a_write_view_before_file_effects() {
    let state = tempfile::tempdir().unwrap();
    let body: conduit_body::BodyId = serde_json::from_str("\"fixture/restart-body\"").unwrap();
    let (mut host, plan, _) = planned(true, "fixture/wrong-view-boot");
    assert!(conduit_environment_linux::recover_durable_catalog_seed(
        &mut host,
        &body,
        state.path(),
        plan.fragments[0].clone(),
        &mut NoWait,
    )
    .is_err());
    assert_eq!(std::fs::read_dir(state.path()).unwrap().count(), 0);
}
