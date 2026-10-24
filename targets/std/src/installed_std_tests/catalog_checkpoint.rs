//! Actual publication-to-seed execution. JSON, resource offers and independent
//! grants are CPU fixtures; file effects and the ordinary installed kernel are real.
use super::catalog_seed::{configured_host, plan_on_host, Collector};
use super::RecordingTimer;
use crate::{ExternalForeInput, ExternalForeRun};
use conduit_core as c;
use conduit_linux_snapshot_store as files;
const CONTENT: &str = "{\"fixture\":\"catalog\"}";
fn reference(version: u8) -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([11; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: CONTENT.len() as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([version; 32]),
            expires_at: None,
        },
    }
}
fn planned(reference: &c::BoundedResourceRef, boot: &str) -> (crate::StdHost, c::Plan, Vec<u8>) {
    let hex: String = reference
        .encode()
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let source = include_str!("../../../../bodies/environment/profiles/catalog-checkpoint.conduit")
        .replace("@REFERENCE@", &hex);
    let mut startup = conduit_plot::StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    startup
        .insert_value_kind_alias("JsonText", c::kind_id("text/json-utf8@1"))
        .unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profiles)
        .unwrap();
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
    startup
        .insert_fore(kind.kind_id.as_str(), kind.checked_front())
        .unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "recorder-catalog", &profiles)
            .unwrap();
    let (mut host, value_type) = configured_host(&authoring, boot);
    let present = authoring
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() != c::STATE_VALUE_KIND && gear.kind_id != kind.kind_id)
        .unwrap();
    let c::ConfigurationValue::Text(program) = &present.configuration[0].value else {
        panic!("program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    host.advertisement.capabilities.push(
        conduit_std_offers::pure_expression_std_offer(&program, c::PortTemporal::Value).unwrap(),
    );
    let resource = c::ResourceOffer {
        pool_id: "fixture/snapshot-files".into(),
        class_id: reference.access_class.clone(),
        capacity_units: 1,
        compute: None,
        content: Some(c::ResourceContentOffer {
            contract: c::ResourceContentRequirement {
                identity: reference.identity,
                version: reference.lifetime.version,
                content_profile: reference.content_profile.clone(),
                maximum_bytes: 4096,
                maximum_items: 1,
                retention: c::ResourceRetention::ExternalDurable,
                sharing: c::ResourceSharing::SingleWriterPublished,
                access: c::ResourceAccessMode::WriteCandidatePublish,
                generation_slots: 1,
                reader_leases: 1,
                publication_slots: 1,
                sensitive: false,
            },
            owner_host: host.advertisement.host_id.clone(),
            owner_boot: host.advertisement.boot_id.clone(),
            base_id: "fixture/snapshot-base".into(),
            residence_profile: crate::linux_snapshot::PROFILE.into(),
        }),
    };
    let offer = crate::linux_snapshot::offer_for_resource(&resource).unwrap();
    let grant = c::AuthorityGrant {
        grant_id: "fixture/snapshot-grant".into(),
        contract_id: files::AUTHORITY_CONTRACT.into(),
        host_call_contract_id: files::PUBLISH_OPERATION.into(),
        subject_kind: kind.kind_id,
        host_id: host.advertisement.host_id.clone(),
        boot_id: host.advertisement.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    host.advertisement.bases.push(c::BaseProviderAdvertisement {
        base_id: "fixture/snapshot-base".into(),
        provider_instance_id: "fixture/snapshot-provider".into(),
        provider_generation: 1,
        implementation_id: "conduit.base/local@1".into(),
        mechanism_family: "fixture/snapshot-files".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
        lifecycle: c::BaseLifecycle::Ready,
        capability_ids: vec![offer.capability_id.clone()],
        resource_pool_ids: vec![resource.pool_id.clone()],
    });
    host.advertisement.capabilities.push(offer);
    host.advertisement.resources.push(resource);
    let plan = plan_on_host(&authoring, &mut host, &[grant], 4096);
    let payload = c::StructuredInfoValue::leaf(
        c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).unwrap(),
        reference.encode().unwrap(),
    )
    .unwrap();
    let selected = c::StructuredInfoValue::variant(value_type, "some", payload)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    (host, plan, selected)
}
#[test]
fn snapshot_publication_and_refusal_control_the_actual_durable_seed() {
    let state = tempfile::tempdir().unwrap();
    let snapshots = tempfile::tempdir().unwrap();
    let directory =
        files::SnapshotDirectory::prepare(std::fs::File::open(snapshots.path()).unwrap()).unwrap();
    let body: conduit_body::BodyId = serde_json::from_str("\"fixture/checkpoint-body\"").unwrap();
    let mut previous = None;
    // An occupied later generation is not a publication receipt for this Play.
    for (index, success) in [true, false].into_iter().enumerate() {
        let selected_reference = reference(12 + index as u8);
        let (mut host, plan, selected) = planned(
            &selected_reference,
            &format!("fixture/checkpoint-boot/{index}"),
        );
        if !success {
            // Simulate a prior file effect without a seed update. Its occupied
            // name is not this new request's successful publication receipt.
            let placement = plan.fragments[0]
                .placements
                .iter()
                .find(|placement| {
                    placement.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND
                })
                .unwrap();
            files::PreparedFileSnapshot::prepare(&directory, placement, &selected_reference)
                .unwrap()
                .publish(&placement.authority[0], CONTENT.as_bytes())
                .unwrap();
        }
        let control = crate::RunControl::default();
        let mut collector = Collector {
            values: vec![],
            stop_after: success.then_some(selected.clone()),
            control: control.clone(),
        };
        let inputs = [ExternalForeInput {
            front_port_id: "catalog".into(),
            track: c::ConnectionTrack::Payload,
            bytes: CONTENT.as_bytes().to_vec(),
        }];
        let result = host.run_body_durable_snapshot_plot_to(
            &body,
            state.path(),
            &directory,
            plan.fragments[0].clone(),
            ExternalForeRun {
                inputs: &inputs,
                output: &mut collector,
                sequential: false,
                stream: None,
            },
            &mut Vec::new(),
            &mut RecordingTimer { waits: vec![] },
            &control,
        );
        if success {
            result.unwrap();
            assert_eq!(collector.values.last().unwrap().bytes.as_slice(), selected);
            let current = std::fs::read_dir(state.path())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path()
                .join("current");
            previous = Some((current.clone(), std::fs::read(current).unwrap()));
            assert_eq!(std::fs::read_dir(snapshots.path()).unwrap().count(), 1);
        } else {
            assert!(result.unwrap_err().contains("PublishedNameExists"));
            assert!(collector
                .values
                .iter()
                .all(|value| value.bytes.as_slice() != selected));
            let (path, bytes) = previous.as_ref().unwrap();
            assert_eq!(&std::fs::read(path).unwrap(), bytes);
        }
    }
}

#[test]
fn snapshot_execution_without_directory_custody_refuses_before_file_effects() {
    let state = tempfile::tempdir().unwrap();
    let (mut host, plan, _) = planned(&reference(15), "fixture/checkpoint-no-directory");
    let body: conduit_body::BodyId = serde_json::from_str("\"fixture/checkpoint-body\"").unwrap();
    let control = crate::RunControl::default();
    let mut collector = Collector::default();
    let inputs = [ExternalForeInput {
        front_port_id: "catalog".into(),
        track: c::ConnectionTrack::Payload,
        bytes: CONTENT.as_bytes().to_vec(),
    }];
    let error = host
        .run_body_durable_external_plot_to(
            &body,
            state.path(),
            plan.fragments[0].clone(),
            ExternalForeRun {
                inputs: &inputs,
                output: &mut collector,
                sequential: false,
                stream: None,
            },
            &mut Vec::new(),
            &mut RecordingTimer { waits: vec![] },
            &control,
        )
        .unwrap_err();
    assert!(error.contains("no admitted directory custody"));
    assert!(collector.values.is_empty());
    assert_eq!(std::fs::read_dir(state.path()).unwrap().count(), 0);
}
