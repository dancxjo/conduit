//! Actual Body-kernel snapshot effects with fixture admission. This does not
//! select an installed owner's recorder, catalog State or a durable-history ACK.
#![cfg(all(feature = "linux-snapshot", target_os = "linux"))]
use conduit_core::*;
use conduit_linux_snapshot_store::SnapshotDirectory;
use conduit_std_host::{body_execution::*, ExternalForeInput, RunControl, StdHost, ThreadTimer};
use std::collections::BTreeMap;
#[path = "common/checkpoint_fixture.rs"]
mod checkpoint_fixture;
use checkpoint_fixture::{body_plan, CapturedFore};
const CONTENT: &[u8] = b"{\"exact-source\":\"fixture\"}";
fn planned(
    write: bool,
) -> (
    StdHost,
    Plan,
    ResourceOffer,
    AuthorityGrant,
    BoundedResourceRef,
) {
    let mut advertisement = StdHost::new().advertisement().clone();
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([71; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: ResourceExtent {
            bytes: CONTENT.len() as u64,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([72; 32]),
            expires_at: None,
        },
    };
    let resource = ResourceOffer {
        pool_id: "snapshot/fixture".into(),
        class_id: reference.access_class.clone(),
        capacity_units: 1,
        compute: None,
        content: Some(ResourceContentOffer {
            contract: ResourceContentRequirement {
                identity: reference.identity,
                version: reference.lifetime.version,
                content_profile: reference.content_profile.clone(),
                maximum_bytes: 4096,
                maximum_items: 1,
                retention: ResourceRetention::ExternalDurable,
                sharing: ResourceSharing::SingleWriterPublished,
                access: if write {
                    ResourceAccessMode::WriteCandidatePublish
                } else {
                    ResourceAccessMode::ReadPublished
                },
                generation_slots: 1,
                reader_leases: 1,
                publication_slots: u16::from(write),
                sensitive: false,
            },
            owner_host: advertisement.host_id.clone(),
            owner_boot: advertisement.boot_id.clone(),
            base_id: "snapshot/base".into(),
            residence_profile: conduit_std_host::linux_snapshot::PROFILE.into(),
        }),
    };
    let offer = conduit_std_host::linux_snapshot::offer_for_resource(&resource).unwrap();
    let requirement = &offer.authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: "snapshot/grant".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    advertisement.bases.push(BaseProviderAdvertisement {
        base_id: "snapshot/base".into(),
        provider_instance_id: "snapshot/provider".into(),
        provider_generation: 1,
        implementation_id: "conduit.base/local@1".into(),
        mechanism_family: "fixture/linux-files".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capability_ids: vec![offer.capability_id.clone()],
        resource_pool_ids: vec![resource.pool_id.clone()],
    });
    advertisement.capabilities.push(offer);
    advertisement.resources.push(resource.clone());
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let (host, plan) = planned_on(host, &reference, &grant, write);
    (host, plan, resource, grant, reference)
}
fn planned_on(
    host: StdHost,
    reference: &BoundedResourceRef,
    grant: &AuthorityGrant,
    write: bool,
) -> (StdHost, Plan) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    startup
        .insert_value_kind_alias("JsonText", kind_id("text/json-utf8@1"))
        .unwrap();
    startup
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profiles)
        .unwrap();
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(write);
    startup
        .insert_fore(kind.kind_id.as_str(), kind.checked_front())
        .unwrap();
    let hex: String = reference
        .encode()
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let (input, output) = if write {
        ("JsonText", "ResourceRef")
    } else {
        ("ResourceRef", "JsonText")
    };
    let source=format!("plot snapshot (\n >> source: {input}\n result: {output} >>\n) {{\n selected: {}(\"{hex}\")\n source >> selected.source\n selected.value >> result\n}}\n",kind.kind_id.as_str());
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "snapshot", &profiles).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let (input_bytes, output_bytes) = if write { (4096, 512) } else { (512, 4096) };
    let boundaries = BTreeMap::from([
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("source"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: input_bytes,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("result"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: output_bytes,
            },
        ),
    ]);
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &authored,
        &profiles,
        &conduit_plot::CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: core::slice::from_ref(grant),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (host, plan)
}
fn thread(test: fn()) {
    std::thread::Builder::new()
        .stack_size(4 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
fn inputs(bytes: &[u8]) -> Vec<ExternalForeInput> {
    vec![ExternalForeInput {
        front_port_id: port_id("source"),
        track: ConnectionTrack::Payload,
        bytes: bytes.to_vec(),
    }]
}
#[test]
fn selected_body_back_publishes_then_independently_reads_exact_bytes() {
    thread(|| {
        let root = tempfile::tempdir().unwrap();
        let directory =
            SnapshotDirectory::prepare(std::fs::File::open(root.path()).unwrap()).unwrap();
        let (mut host, plan, resource, grant, reference) = planned(true);
        let (wake, body) = body_plan(plan);
        let mut fore = CapturedFore::default();
        let control = RunControl::default();
        let mut started = false;
        let report = host
            .run_body_plan_with_snapshot_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &body,
                    control: &control,
                    keyboard: None,
                },
                BodyForeExchange {
                    inputs: &inputs(CONTENT),
                    output: &mut fore,
                },
                BodySnapshotSelection {
                    directory: &directory,
                    resource: &resource,
                    authority: &grant,
                    body_id: &body.body_id,
                    plan_id: &body.plan_id,
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |play, _| {
                    assert_eq!(play.body_id, body.body_id);
                    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
                    started = true;
                    Ok(())
                },
            )
            .unwrap();
        assert!(started);
        assert_eq!(report.terminal, TerminalDisposition::Completed);
        assert!(report.failure.is_none(), "{:?}", report.failure);
        assert!(report.cleanup_failure.is_none());
        assert_eq!(fore.0.len(), 1);
        assert_eq!(fore.0[0].bytes.as_slice(), reference.encode().unwrap());
        let (mut read_host, plan, resource, grant, _) = planned(false);
        let (wake, read_body) = body_plan(plan);
        let mut read_fore = CapturedFore::default();
        let report = read_host
            .run_body_plan_with_snapshot_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &read_body,
                    control: &control,
                    keyboard: None,
                },
                BodyForeExchange {
                    inputs: &inputs(&reference.encode().unwrap()),
                    output: &mut read_fore,
                },
                BodySnapshotSelection {
                    directory: &directory,
                    resource: &resource,
                    authority: &grant,
                    body_id: &read_body.body_id,
                    plan_id: &read_body.plan_id,
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| Ok(()),
            )
            .unwrap();
        assert_eq!(report.terminal, TerminalDisposition::Completed);
        assert!(report.failure.is_none(), "{:?}", report.failure);
        assert_eq!(read_fore.0.len(), 1);
        assert_eq!(read_fore.0[0].bytes.as_slice(), CONTENT);
    });
}
#[test]
fn refused_start_and_missing_custody_never_write_or_deliver() {
    thread(|| {
        let root = tempfile::tempdir().unwrap();
        let directory =
            SnapshotDirectory::prepare(std::fs::File::open(root.path()).unwrap()).unwrap();
        let (mut host, plan, resource, grant, _) = planned(true);
        let (wake, body) = body_plan(plan);
        let control = RunControl::default();
        let mut fore = CapturedFore::default();
        let mut started = false;
        assert!(host
            .run_body_plan_with_fore_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &body,
                    control: &control,
                    keyboard: None
                },
                &inputs(CONTENT),
                false,
                &mut fore,
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| {
                    started = true;
                    Ok(())
                }
            )
            .is_err());
        assert!(!started);
        assert!(host
            .run_body_plan_with_snapshot_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &body,
                    control: &control,
                    keyboard: None
                },
                BodyForeExchange {
                    inputs: &inputs(CONTENT),
                    output: &mut fore
                },
                BodySnapshotSelection {
                    directory: &directory,
                    resource: &resource,
                    authority: &grant,
                    body_id: &body.body_id,
                    plan_id: &body.plan_id
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| Err("durable start refused".into())
            )
            .is_err());
        assert!(fore.0.is_empty());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        // The single original pool unit must have been released on callback
        // refusal. Re-admission succeeds without a second or enlarged grant.
        let report = host
            .run_body_plan_with_snapshot_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &body,
                    control: &control,
                    keyboard: None,
                },
                BodyForeExchange {
                    inputs: &inputs(CONTENT),
                    output: &mut fore,
                },
                BodySnapshotSelection {
                    directory: &directory,
                    resource: &resource,
                    authority: &grant,
                    body_id: &body.body_id,
                    plan_id: &body.plan_id,
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| Ok(()),
            )
            .unwrap();
        assert_eq!(report.terminal, TerminalDisposition::Completed);
        assert!(report.failure.is_none());
        assert_eq!(fore.0.len(), 1);
    });
}
#[test]
fn foreign_grant_refuses_before_start() {
    thread(|| {
        for wrong_boot in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let directory =
                SnapshotDirectory::prepare(std::fs::File::open(root.path()).unwrap()).unwrap();
            let (mut host, plan, resource, mut grant, _) = planned(true);
            let (wake, body) = body_plan(plan);
            if wrong_boot {
                grant.boot_id = "boot/foreign".into();
            } else {
                grant.grant_id = "grant/not-selected".into();
            }
            let mut fore = CapturedFore::default();
            let control = RunControl::default();
            let mut started = false;
            let result = host.run_body_plan_with_snapshot_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &body,
                    control: &control,
                    keyboard: None,
                },
                BodyForeExchange {
                    inputs: &inputs(CONTENT),
                    output: &mut fore,
                },
                BodySnapshotSelection {
                    directory: &directory,
                    resource: &resource,
                    authority: &grant,
                    body_id: &body.body_id,
                    plan_id: &body.plan_id,
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| {
                    started = true;
                    Ok(())
                },
            );
            assert!(result.is_err());
            assert!(!started);
            assert!(fore.0.is_empty());
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        }
    });
}

#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
#[path = "body_snapshot_play/selected_storage.rs"]
mod selected_storage;
