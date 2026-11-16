//! Actual checked IMAGE counter and OS-owned File capture, not copied receipts.
use super::*;
use conduit_core as c;
use conduit_host_make as h;
use conduit_std_host::{ImmediateSourcePreparationBudget, RunControl, StdHost, StdHostConfig};
const SOURCE: &str = include_str!("../../../../make/body/tests/source_check_call/check.conduit");
const CALLER: usize = 16 * 1024 * 1024;
struct RealCapture {
    prepared: PreparedFileSource,
    selection: Selection,
    biography: BodyBiographyEvidence,
    path: std::path::PathBuf,
    configuration: super::super::selected_host_configuration::Selection,
}
fn real_capture() -> RealCapture {
    let executable = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_RETAINED_SOURCE_TEST_COMPILER")
            .expect("explicit independently selected Source worker executable"),
    );
    let sha = format!("{:x}", Sha256::digest(fs::read(&executable).unwrap()));
    let text = format!(
        r#"host compiler {{
 schema = 1
 target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
 base = {{kind: "language/source-compiler", implementation: "std/bounded-source-check@1", initialization: {{executable: {}, executable_sha256: "{sha}", maximum_executable_bytes: 536870912, address_space_bytes: 536870912, cpu_seconds: 10, deadline_milliseconds: 20000, maximum_startup_bytes: 16384, preparation_bytes: 570425344, retained_bytes: 1048576, request_bytes: 16384, reply_bytes: 65536}}}}
 base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
 capability = {{kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}}
 capability = {{kind: "language/source-check", pinned_implementation: "std/bounded-source-check@1"}}
 need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}}
 need = {{id: "source/results", class: "language/prepared-source-check@1", slots: 1, bytes: 1048576}}
 need = {{id: "source/memory", class: "conduit.resource/runtime-memory@1", slots: 570425344, bytes: 570425344}}
 limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 603979776, queue_items: 32, buffered_bytes: 1048576, active_instances: 16, operation_slots: 16, timer_slots: 16, line_sessions: 16, evidence_items: 16}}
}}"#,
        serde_json::to_string(executable.to_str().unwrap()).unwrap()
    );
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = h::check_host_configuration(
        h::parse_host_configuration_conduit(&text).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = h::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &h::BuildInputs {
            source_identity: "fixture/retained-source-image".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let config = StdHostConfig {
        host_id: "host/retained-source-test".into(),
        boot_id: super::super::fresh_identity("boot/retained-source", "test").into(),
        offer_generation: c::OfferGeneration(1),
    };
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let body = conduit_body::Body::born(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
        1,
        "fixture/born".into(),
    )
    .unwrap();
    // Genuine local birth membership from the preceding Boot; resume_here
    // must perform its ordinary continuity transition rather than bypass it.
    let mut membership = conduit_body::BodyMembership::new(body.body_id.clone()).unwrap();
    let part = conduit_body::PartId::bind(&body.body_id, config.host_id.as_str(), 0).unwrap();
    let proof = conduit_body::MembershipProofId::bind("fixture/local-birth").unwrap();
    let mut biography =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Retained Source".into())
            .unwrap();
    let sign = |sequence| {
        c::bind_sign(
            &config.host_id,
            &"boot/prior-retained-source".into(),
            None,
            sequence,
        )
        .sign_id
    };
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            sign(2),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            conduit_body::AuthenticatedHostObservation {
                host_id: config.host_id.clone(),
                boot_id: "boot/prior-retained-source".into(),
                offer_generation: c::OfferGeneration(1),
                proof_id: proof,
                sequence: 0,
            },
            sign(3),
        )
        .unwrap();
    biography
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let path = std::env::temp_dir()
        .join(super::super::fresh_identity("retained-source-file", "test").replace('/', "-"));
    fs::write(&path, SOURCE).unwrap();
    let projection = conduit_plot::SourceCheckProjection::new(
        "fixture/owned-file",
        1,
        SOURCE,
        conduit::source_check_worker::standard_context().unwrap(),
    )
    .unwrap();
    let selection:Selection=serde_json::from_value(serde_json::json!({
        "host_id":config.host_id,"configuration_id":checked.configuration_id(),"release_bundle_sha256":format!("sha256:{}","1".repeat(64)),"expected":biography,"file":path,"snapshot":projection.snapshot(),"entries":["check-source"],"maximum_source_bytes":16384,"capture_preparation_bytes":524288,"capture_retained_bytes":65536,"caller_workspace_bytes":CALLER,
        "local_fore_adapters":[{"entry":"check-source","host_id":"member/target","configuration_id":"fixture/target-config","ports":plot.front.inputs().iter().chain(plot.front.outputs()).map(|port|serde_json::json!({"front_port_id":port.port_id,"direction":port.direction,"track":c::ConnectionTrack::Payload,"value_kind":port.value_kind,"item_capacity":1,"byte_capacity":16384})).collect::<Vec<_>>()}]
    })).unwrap();
    let grant = c::AuthorityGrant {
        grant_id: "grant/actual-os-file".into(),
        contract_id: conduit_std_offers::SOURCE_CHECK_AUTHORITY.into(),
        host_call_contract_id: conduit_std_offers::SOURCE_CHECK_CALL.into(),
        subject_kind: conduit_std_offers::SOURCE_CHECK_KIND.into(),
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: conduit_std_offers::SOURCE_CHECK_IMPLEMENTATION.into(),
    };
    // SAFETY: this fixture owns the actual File and explicit Host/Boot grant.
    // Original IMAGE construction mints real capture/caller memory leases.
    let initialized = unsafe {
        conduit::source_check_worker::initialize_image_with_caller_workspace(
            conduit::source_check_worker::SourceCheckImageInstallation {
                config,
                image: &image,
                checked: &checked,
                catalog: &catalog,
                grant,
                provider: conduit::source_check_worker::SourceCheckProviderIdentity {
                    instance: "fixture/original-file-provider".into(),
                    generation: 1,
                    resource_generation: c::ResourceGenerationId(
                        "fixture/original-resource".into(),
                    ),
                },
                file: fs::File::open(&path).unwrap(),
                maximum_source_bytes: 16384,
                capture_budget: ImmediateSourcePreparationBudget {
                    preparation_bytes: 524288,
                    retained_bytes: 65536,
                },
                snapshot: &selection.snapshot,
                entry: &selection.entries[0],
                context: projection.context().clone(),
                control: &RunControl::default(),
            },
            None,
            Some(CALLER),
            None,
        )
    }
    .unwrap();
    RealCapture {
        prepared: PreparedFileSource {
            host: initialized.host,
            source: initialized.source,
            caller: initialized.caller.unwrap(),
            caller_workspace_bytes: CALLER,
        },
        selection,
        biography,
        path,
        configuration: serde_json::from_value(serde_json::json!({
            "source": text, "configuration_id": checked.configuration_id(), "host_image": image,
        }))
        .unwrap(),
    }
}
#[test]
#[ignore = "requires explicit digest-pinned product Source worker artifact"]
fn retained_fore_original_image_custody_restores_current_workset_and_reselects_fresh_offers() {
    let RealCapture {
        prepared,
        selection,
        biography,
        path,
        ..
    } = real_capture();
    let (host, mut custody) = prepared.retain_for_restart(&selection).unwrap();
    let other = StdHost::new_with_config(StdHostConfig {
        host_id: host.advertisement().host_id.clone(),
        boot_id: host.advertisement().boot_id.clone(),
        offer_generation: host.advertisement().offer_generation,
    });
    assert!(
        custody.verify_capture(&other).is_err(),
        "copied labels have no original IMAGE counter"
    );
    let snapshot = custody.snapshot.clone();
    custody.snapshot.generation += 1;
    assert!(custody.verify_capture(&host).is_err());
    custody.snapshot = snapshot;
    let mut changed = biography.clone();
    let other_source =
        crate::plot_source::parse(include_str!("../../../../plots/hello/main.conduit"))
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
    changed.body.workset =
        conduit_body::BodyWorkset::from_plots([conduit_body::ResidentPlot::new(
            other_source.expanded.source_document_id,
            other_source.expanded.checked_plot_id,
        )])
        .unwrap();
    assert!(custody.check_current(&host, &changed).is_err());
    let probe = crate::allocation_probe::begin();
    let retained = custody.restore(&host, &biography).unwrap();
    let measured = probe.finish_statistics();
    eprintln!("retained-source restore caller requested-byte observation: {measured:?}");
    assert!(
        measured.peak_live_bytes <= CALLER,
        "restore requested-byte peak exceeds original caller: {measured:?}"
    );
    assert!(retained
        .select_current_local_fore(&host, &biography, &[])
        .is_err());
    let mut target = host.advertisement().clone();
    target.host_id = "member/target".into();
    target.boot_id = "member/fresh-boot".into();
    target.profile = "fixture/target-config".into();
    target.offer_generation = c::OfferGeneration(9);
    let selected = retained
        .select_current_local_fore(&host, &biography, std::slice::from_ref(&target))
        .unwrap();
    assert!(selected
        .verify(host.advertisement(), std::slice::from_ref(&target))
        .is_ok());
    target.offer_generation = c::OfferGeneration(10);
    assert!(selected
        .verify(host.advertisement(), std::slice::from_ref(&target))
        .is_err());
    let reselected = retained
        .select_current_local_fore(&host, &biography, std::slice::from_ref(&target))
        .unwrap();
    assert!(reselected
        .verify(host.advertisement(), std::slice::from_ref(&target))
        .is_ok());
    let wrong = conduit_body::BodyWorkset::from_plots([conduit_body::ResidentPlot::new(
        "source/wrong".into(),
        "plot/wrong".into(),
    )])
    .unwrap();
    assert!(retained.verify(&host, &wrong).is_err());
    assert!(retained.verify(&other, &biography.body.workset).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), SOURCE);
    fs::remove_file(path).unwrap();
}

pub(crate) fn real_owner_custody() -> (
    StdHost,
    RetainedSourcePreparation,
    BodyBiographyEvidence,
    std::path::PathBuf,
) {
    let RealCapture {
        prepared,
        selection,
        biography,
        path,
        ..
    } = real_capture();
    let (host, custody) = prepared.retain_for_restart(&selection).unwrap();
    (host, custody, biography, path)
}

/// Genuine original IMAGE/worker fixture with its exact OS-selected policy.
/// Configuration JSON is correspondence data, never a substitute for guards.
pub(crate) fn real_owner_state() -> (
    crate::durable_host::owner::Owner,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    use std::os::unix::fs::PermissionsExt;
    let RealCapture {
        prepared,
        selection,
        biography,
        path,
        configuration,
    } = real_capture();
    let root = path.with_extension("execution-state");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let biography_path = root.join("original-biography.json");
    let biography_bytes = serde_json::to_vec(&biography).unwrap();
    fs::write(&biography_path, &biography_bytes).unwrap();
    let installation = super::super::Installation {
        schema: super::super::INSTALL_SCHEMA.into(),
        host_id: selection.host_id.clone(),
        release_source_identity: "fixture/retained-source-image".into(),
        release_bundle_sha256: selection.release_bundle_sha256.clone(),
        product_executable: "fixture-unused".into(),
        body_state: Some(super::super::BodyBinding {
            body_id: biography.body_id.as_str().into(),
            biography_sha256: super::super::digest(&biography_bytes),
            biography_path: biography_path.to_str().unwrap().into(),
        }),
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: Some(configuration),
        selected_todo_checkpoint: None,
    };
    super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    super::super::write_json_atomic(&root.join(SELECTION_FILE), &selection).unwrap();
    let (host, custody) = prepared.retain_for_restart(&selection).unwrap();
    let mut owner = crate::durable_host::owner::resume_retained_source_test_owner(host, biography).unwrap();
    owner.restore_selected_file_planning(custody).unwrap();
    (owner, root, path)
}
