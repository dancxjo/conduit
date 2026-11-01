//! Actual composed Body publication -> authored Cord -> Body State checkpoint.
//! Admission and finite filesystem/workspace envelope are fixture-selected;
//! this is neither installed owner admission nor a domain history ACK.
#![cfg(target_os = "linux")]
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core as c;
use conduit_environment_make::RecordingCatalogSeedSource;
use conduit_std_host::{self as host, body_execution::*};
#[path = "support/catalog_host.rs"]
mod catalog_host;

const CONTENT_LIMIT: u32 = 4096;
const STATE_LIMIT: u32 = 1024;
const SNAPSHOT_DISK_LIMIT: usize = conduit_resource_snapshot::MAXIMUM_RECORD_BYTES;
const STATE_DISK_LIMIT: usize = 2 * (STATE_LIMIT as usize + 128);
const MAXIMUM_FILES: usize = 3;

fn reference(identity: u8, bytes: usize) -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([identity; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: bytes as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([identity + 1; 32]),
            expires_at: None,
        },
    }
}
fn resource(
    reference: &c::BoundedResourceRef,
    advertisement: &c::HostAdvertisement,
) -> c::ResourceOffer {
    c::ResourceOffer {
        pool_id: "fixture/catalog-snapshot".into(),
        class_id: reference.access_class.clone(),
        capacity_units: 1,
        compute: None,
        content: Some(c::ResourceContentOffer {
            contract: c::ResourceContentRequirement {
                identity: reference.identity,
                version: reference.lifetime.version,
                content_profile: reference.content_profile.clone(),
                maximum_bytes: CONTENT_LIMIT,
                maximum_items: 1,
                retention: c::ResourceRetention::ExternalDurable,
                sharing: c::ResourceSharing::SingleWriterPublished,
                access: c::ResourceAccessMode::WriteCandidatePublish,
                generation_slots: 1,
                reader_leases: 1,
                publication_slots: 1,
                sensitive: false,
            },
            owner_host: advertisement.host_id.clone(),
            owner_boot: advertisement.boot_id.clone(),
            base_id: "fixture/catalog-files".into(),
            residence_profile: host::linux_snapshot::PROFILE.into(),
        }),
    }
}
fn planned_checkpoint(
    reference: &c::BoundedResourceRef,
    boot: &str,
) -> (
    host::StdHost,
    c::Plan,
    c::ResourceOffer,
    c::ResourceOffer,
    c::AuthorityGrant,
    c::StructuredInfoType,
) {
    let source = RecordingCatalogSeedSource::compile_checkpoint(reference).unwrap();
    let mut advertisement = catalog_host::configured(&source, boot);
    let present = source
        .plot
        .expanded
        .gears
        .iter()
        .find(|gear| {
            gear.kind_id.as_str() != c::STATE_VALUE_KIND
                && gear.kind_id.as_str() != conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND
        })
        .unwrap();
    let c::ConfigurationValue::Text(program) = &present.configuration[0].value else {
        panic!("checked some program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    // Select the existing finite kernel-local evaluator explicitly; no new
    // platform Host Call or special-case optional-reference conversion.
    advertisement
        .capabilities
        .push(conduit_std_offers::pure_expression_local_offer(&program).unwrap());

    let snapshot = resource(reference, &advertisement);
    let offer = host::linux_snapshot::offer_for_resource(&snapshot).unwrap();
    let requirement = &offer.authority_requirements[0];
    let grant = c::AuthorityGrant {
        grant_id: format!("fixture/catalog-grant/{boot}").into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    advertisement.bases.push(c::BaseProviderAdvertisement {
        base_id: "fixture/catalog-files".into(),
        provider_instance_id: "fixture/catalog-files-provider".into(),
        provider_generation: 1,
        implementation_id: "conduit.base/local@1".into(),
        mechanism_family: "fixture/linux-files".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
        lifecycle: c::BaseLifecycle::Ready,
        capability_ids: vec![offer.capability_id.clone()],
        resource_pool_ids: vec![snapshot.pool_id.clone()],
    });
    advertisement.capabilities.push(offer);
    advertisement.resources.push(snapshot.clone());
    let state = advertisement
        .resources
        .iter()
        .find(|r| r.class_id.as_str() == conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS)
        .unwrap()
        .clone();
    let (host, plan) = catalog_host::plan_source(
        &source,
        advertisement,
        std::slice::from_ref(&grant),
        CONTENT_LIMIT,
    );
    assert_eq!(plan.fragments.len(), 1);
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.states.len(), 1);
    assert_eq!(fragment.states[0].maximum_value_bytes, STATE_LIMIT);
    let pools: Vec<_> = fragment
        .placements
        .iter()
        .flat_map(|p| &p.resources)
        .map(|r| (r.pool_id.clone(), r.units))
        .collect();
    assert_eq!(
        pools
            .iter()
            .filter(|(pool, _)| pool == &snapshot.pool_id)
            .count(),
        1
    );
    assert_eq!(
        pools
            .iter()
            .filter(|(pool, _)| pool == &state.pool_id)
            .count(),
        1
    );
    assert!(pools.iter().all(|(_, units)| *units == 1));
    // The original runner reserves complete sealed placement resources and
    // preparation/effect closure; 4KiB payload alone is not the envelope.
    let profile = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, STATE_LIMIT)
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment_for_profile_from_plan(
        &plan,
        &fragment.fragment_id,
        profile,
    )
    .unwrap();
    assert!(lowered.cord_value_bytes <= 16 * 1024 * 1024);
    assert!(lowered.sign_bytes <= 4 * 1024 * 1024);
    assert!(lowered
        .host_calls
        .iter()
        .all(|call| call.binding.maximum_input_bytes <= CONTENT_LIMIT
            && call.binding.maximum_output_bytes <= CONTENT_LIMIT));
    (host, plan, snapshot, state, grant, source.value_type)
}
fn resident(plan: &c::Plan) -> ResidentPlot {
    ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    )
}
fn seal(body: &Body, plan: c::Plan, sequence: u64) -> (Body, conduit_body::Wake, BodyPlan) {
    let (awake, wake) = body
        .wake(sequence, format!("fixture/wake/{sequence}").into())
        .unwrap();
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident(&plan),
            plan,
        }],
    )
    .unwrap();
    (awake, wake, plan)
}
fn lull(body: &Body, report: &BodyRunReport) -> Body {
    let wake = report.wake_at_start.lull("fixture/lull".into()).unwrap();
    body.retain_after_lull(&wake, "fixture/retained".into())
        .unwrap()
}
fn read_body(
    body: &Body,
    old: &ResidentPlot,
) -> (
    host::StdHost,
    Body,
    conduit_body::Wake,
    BodyPlan,
    c::ResourceOffer,
) {
    let (host, plan, _) = catalog_host::planned(false, "fixture/catalog-recovered-boot");
    let next = body
        .admit_plot(resident(&plan), "fixture/admit-read".into())
        .unwrap()
        .remove_plot(old, "fixture/remove-write".into())
        .unwrap();
    assert_eq!(body.body_id, next.body_id);
    let state = host
        .advertisement()
        .resources
        .iter()
        .find(|r| r.class_id.as_str() == conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS)
        .unwrap()
        .clone();
    let (awake, wake, plan) = seal(&next, plan, 2);
    (host, awake, wake, plan, state)
}
struct Capture {
    values: Vec<Vec<u8>>,
    sabotage: Option<std::path::PathBuf>,
    stop: Option<host::RunControl>,
}
impl BodyBorrowedForeOutputAdapter for Capture {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if self.values.len() == 2 {
            return Err("fixture independently admitted two outputs".into());
        }
        if let Some(root) = self.sabotage.take() {
            let entries: Vec<_> = std::fs::read_dir(root)
                .map_err(|e| e.to_string())?
                .map(|e| e.unwrap().path())
                .collect();
            if entries.len() != 1 {
                return Err("fixture State residence not opened exactly once".into());
            }
            // Effect fault injection after recovery/current delivery: publication
            // remains an independent effect, but the later State candidate write fails.
            std::fs::create_dir(entries[0].join("candidate")).map_err(|e| e.to_string())?;
        }
        self.values.push(value.bytes.to_vec());
        if let Some(control) = self.stop.take() {
            control
                .request_stop(host::RunControlRequestId::new("fixture/first-current").unwrap())
                .map_err(|e| format!("fixture stop: {e:?}"))?;
        }
        Ok(())
    }
}
fn decoded_reference(bytes: &[u8]) -> Option<c::BoundedResourceRef> {
    let value = c::StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    match value.shape() {
        c::StructuredInfoValueShape::Variant { tag, payload } if tag == "some" => {
            match payload.shape() {
                c::StructuredInfoValueShape::Leaf(bytes) => {
                    Some(c::BoundedResourceRef::decode(bytes).unwrap())
                }
                _ => panic!("reference leaf"),
            }
        }
        c::StructuredInfoValueShape::Variant { tag, .. } if tag == "none" => None,
        _ => panic!("optional reference"),
    }
}
#[path = "../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
mod snapshot_fixture;
use conduit_resource_snapshot::{PUBLISH_OPERATION, READ_OPERATION};
fn independently_read(
    directory: &conduit_linux_snapshot_store::SnapshotDirectory,
    reference: &c::BoundedResourceRef,
    owner: &c::HostId,
) -> Vec<u8> {
    let (mut placement, _) = snapshot_fixture::placement(
        "fixture/independent-read-boot",
        false,
        reference.extent.bytes as usize,
    );
    placement.host_id = owner.clone();
    placement.authority[0].host_id = owner.clone();
    let content = placement.resources[0].content.as_mut().unwrap();
    content.owner_host = owner.clone();
    content.contract.identity = reference.identity;
    content.contract.version = reference.lifetime.version;
    let mut selected = conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(
        directory, &placement, reference,
    )
    .unwrap();
    selected.read(&placement.authority[0]).unwrap().to_vec()
}
fn check_disk(root: &std::path::Path, maxbytes: usize, maxfiles: usize) {
    fn scan(root: &std::path::Path) -> (usize, usize) {
        let mut total = (0, 0);
        for item in std::fs::read_dir(root).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                let part = scan(&path);
                total.0 += part.0;
                total.1 += part.1;
            } else {
                total.0 += std::fs::metadata(path).unwrap().len() as usize;
                total.1 += 1;
            }
        }
        total
    }
    let (bytes, files) = scan(root);
    assert!(bytes <= maxbytes);
    assert!(files <= maxfiles);
}
fn thread(test: fn()) {
    std::thread::Builder::new()
        .stack_size(4 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn actual_body_checkpoint_routes_publication_to_state_and_fresh_boot_recovers_first() {
    thread(|| run(false));
}
#[test]
fn published_snapshot_is_not_catalog_state_commit_after_state_effect_failure() {
    thread(|| run(true));
}
fn run(fail_state: bool) {
    let snapshot_root = tempfile::tempdir().unwrap();
    let state_root = tempfile::tempdir().unwrap();
    let directory = conduit_linux_snapshot_store::SnapshotDirectory::prepare(
        std::fs::File::open(snapshot_root.path()).unwrap(),
    )
    .unwrap();
    // These are opaque fixture generation references, not invented commit
    // observers. This test owns catalogue publication/state, not chunk recording.
    let catalog = conduit_environment_runtime::recording::PreparedRecordingCatalog::prepare(
        &[reference(31, 64)],
        2,
        8192,
    )
    .unwrap();
    let reference = reference(41, catalog.content().len());
    let (mut host, plan, snapshot, state, grant, _) =
        planned_checkpoint(&reference, "fixture/catalog-write-boot");
    let old = resident(&plan);
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "fixture/born".into(),
    )
    .unwrap();
    let (awake, wake, plan) = seal(&body, plan, 1);
    let mut capture = Capture {
        values: Vec::with_capacity(2),
        sabotage: fail_state.then(|| state_root.path().to_path_buf()),
        stop: None,
    };
    let inputs = [host::ExternalForeInput {
        front_port_id: "catalog".into(),
        track: c::ConnectionTrack::Payload,
        bytes: catalog.content().as_bytes().to_vec(),
    }];
    let report = host
        .run_body_plan_with_durable_state_borrowed_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &host::RunControl::default(),
                keyboard: None,
            },
            &inputs,
            &mut capture,
            BodyDurableStateSelection {
                root: state_root.path(),
                resource: &state,
                authorities: &[],
                body_id: &plan.body_id,
                plan_id: &plan.plan_id,
            },
            Some(BodySnapshotSelection {
                directory: &directory,
                resource: &snapshot,
                authority: &grant,
                body_id: &plan.body_id,
                plan_id: &plan.plan_id,
            }),
            &mut Vec::new(),
            &mut catalog_host::NoWait,
            |play, _| {
                assert!(play.validate_for(&plan));
                assert_eq!(std::fs::read_dir(snapshot_root.path()).unwrap().count(), 0);
                assert_eq!(std::fs::read_dir(state_root.path()).unwrap().count(), 0);
                Ok(())
            },
        )
        .unwrap();
    assert!(
        report.failure.is_none() || fail_state,
        "{:?} / {:?}",
        report.failure,
        report.kernel_failure
    );
    if fail_state {
        assert_eq!(
            report.kernel_failure,
            Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallFailed,
                detail: 7
            }),
            "{:?}",
            report.failure
        );
    }
    assert!(
        report.cleanup_failure.is_none(),
        "{:?}",
        report.cleanup_failure
    );
    assert_eq!(
        independently_read(
            &directory,
            &reference,
            &plan.plots[0].plan.fragments[0].host_id
        ),
        catalog.content().as_bytes()
    );
    let mut decoded =
        conduit_environment_runtime::recording::PreparedRecordingCatalogReader::default();
    decoded.read(catalog.content()).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded_reference(capture.values.first().unwrap()), None);
    if fail_state {
        assert!(report.failure.is_some());
        assert_eq!(
            report.kernel_failure,
            Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallFailed,
                detail: 7
            })
        );
        assert!(matches!(
            report.terminal,
            c::TerminalDisposition::Failed { .. }
        ));
        assert_eq!(capture.values.len(), 1);
        // The deliberately obstructed candidate is not a committed generation;
        // fresh recovery must ignore it without any repair or promotion.
    } else {
        assert_eq!(
            report.terminal,
            c::TerminalDisposition::Completed,
            "{:?}",
            report.failure
        );
        assert!(report.failure.is_none());
        assert_eq!(
            decoded_reference(capture.values.last().unwrap()),
            Some(reference.clone())
        );
    }
    check_disk(snapshot_root.path(), SNAPSHOT_DISK_LIMIT, 1);
    check_disk(state_root.path(), STATE_DISK_LIMIT, MAXIMUM_FILES - 1);
    let retained = lull(&awake, &report);
    drop(host);
    let (mut fresh, _, wake, read_plan, resource) = read_body(&retained, &old);
    assert_ne!(
        read_plan.plots[0].plan.fragments[0].boot_id,
        plan.plots[0].plan.fragments[0].boot_id
    );
    assert_eq!(
        read_plan.plots[0].plan.fragments[0].states[0].state_id,
        plan.plots[0].plan.fragments[0].states[0].state_id
    );
    let read_control = host::RunControl::default();
    let mut recovered = Capture {
        values: Vec::with_capacity(2),
        sabotage: None,
        stop: Some(read_control.clone()),
    };
    let result = fresh
        .run_body_plan_with_durable_state_borrowed_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &read_plan,
                control: &read_control,
                keyboard: None,
            },
            &[],
            &mut recovered,
            BodyDurableStateSelection {
                root: state_root.path(),
                resource: &resource,
                authorities: &[],
                body_id: &read_plan.body_id,
                plan_id: &read_plan.plan_id,
            },
            None,
            &mut Vec::new(),
            &mut catalog_host::NoWait,
            |_, _| Ok(()),
        )
        .unwrap();
    assert!(
        matches!(result.terminal, c::TerminalDisposition::Cancelled { .. }),
        "{:?}",
        result
    );

    assert!(result.cleanup_failure.is_none());
    assert_eq!(recovered.values.len(), 1);
    assert_eq!(
        decoded_reference(&recovered.values[0]),
        if fail_state { None } else { Some(reference) }
    );
}
