//! Descriptive/correspondence tests only; no IMAGE/Ready or disk-capacity claim.
use super::*;
use crate as conduit_std_host;
fn selected(write: bool) -> (NativeTextFileDefinition, BoundedResourceRef) {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([17; 32]),
        content_profile: kind_id("value/text"),
        access_class: DURABLE_TEXT_RESOURCE.into(),
        extent: ResourceExtent {
            bytes: 11,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([19; 32]),
            expires_at: None,
        },
    };
    let host: HostId = "host/selected-file".into();
    let boot: BootId = "boot/selected-file".into();
    let base_id: HostBaseId = "selected-file".into();
    let content = ResourceContentRequirement {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        maximum_bytes: 128,
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
    };
    let offer = durable_text_offer(content.clone()).unwrap();
    let grant = AuthorityGrant {
        grant_id: "grant/selected-file".into(),
        contract_id: DURABLE_TEXT_AUTHORITY.into(),
        host_call_contract_id: offer.host_calls[0].contract_id.clone(),
        subject_kind: offer.kind_id.clone(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let d = NativeTextFileDefinition {
        base: BaseProviderEntry {
            base_id: base_id.clone(),
            provider_instance_id: "base/selected-file".into(),
            provider_generation: 1,
            implementation_id: offer.implementation.implementation_id.as_str().into(),
            mechanism_family: DURABLE_TEXT_BASE_KIND.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![ResourceOffer {
                pool_id: "pool/file-generation".into(),
                class_id: reference.access_class.clone(),
                capacity_units: 1,
                compute: None,
                content: Some(ResourceContentOffer {
                    contract: content,
                    owner_host: host,
                    owner_boot: boot,
                    base_id,
                    residence_profile: DURABLE_TEXT_RESIDENCE.into(),
                }),
            }],
        },
        authority: grant,
        maximum_text_bytes: 128,
        preparation_bytes: 32768,
        retained_bytes: 16384,
    };
    (d, reference)
}
#[test]
fn descriptive_scope_requires_exact_original_reference_and_independent_operation_grant() {
    for write in [true, false] {
        let (d, r) = selected(write);
        let host = d.authority.host_id.clone();
        let boot = d.authority.boot_id.clone();
        validate_definition(&d, &r, &host, &boot).unwrap();
        let mut other = r.clone();
        other.lifetime.version = ResourceVersionIdentity::from_digest([23; 32]);
        assert!(validate_definition(&d, &other, &host, &boot).is_err());
        let (mut d, r) = selected(write);
        d.authority.boot_id = "boot/foreign".into();
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
        let (mut d, r) = selected(write);
        d.authority.subject_kind = "value/text".into();
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
        let (mut d, r) = selected(write);
        d.base.resources[0].content.as_mut().unwrap().base_id = "another-base".into();
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
    }
}
#[test]
fn descriptive_provider_cannot_enlarge_its_declared_bounds_or_residence() {
    for case in 0..5 {
        let (mut d, r) = selected(true);
        let host = d.authority.host_id.clone();
        let boot = d.authority.boot_id.clone();
        match case {
            0 => d.retained_bytes = d.preparation_bytes + 1,
            1 => d.maximum_text_bytes = 10,
            2 => d.preparation_bytes = 0,
            3 => d.maximum_text_bytes = 0,
            _ => d.base.resources[0].capacity_units = 2,
        }
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
    }
}

fn cold_host(directory: &std::path::Path, heap: u64, write: bool) -> crate::StdHost {
    use conduit_host_make::*;
    let packages = conduit_workspace_make::package_set();
    let catalog = conduit_workspace_make::catalog();
    let source = include_str!("../../profiles/raspberry-pi-3-tick-workset.host.conduit")
        .lines()
        .filter(|line| !line.contains("serial/text") && !line.contains("storage/protected-file"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("  limits =", r#"  base = {kind: "storage/text-generation", implementation: "std/file-data-save-text@1", initialization: {maximum_text_bytes: 128, preparation_bytes: 32768, retained_bytes: 16384}}
  capability = {kind: "data/save/text", pinned_implementation: "std/file-data-save-text@1"}
  need = {id: "recording-generation", class: "data/immutable-generation@1", slots: 1, bytes: 128}
  limits ="#);
    let source = if write {
        source
    } else {
        source
            .replace("std/file-data-save-text@1", "std/file-data-load-text@1")
            .replace("data/save/text", "data/load/text")
    };
    let mut configuration = parse_host_configuration_conduit(&source).unwrap();
    configuration.limits.heap_arena_bytes = heap;
    let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/cold-file".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let selected_base = image
        .manifest
        .base_selections
        .iter()
        .find(|b| b.kind == DURABLE_TEXT_BASE_KIND)
        .unwrap();
    let (mut definition, reference) = selected(write);
    if !write {
        definition.authority.boot_id = "boot/selected-file-reader".into();
        definition.base.resources[0]
            .content
            .as_mut()
            .unwrap()
            .owner_boot = definition.authority.boot_id.clone();
    }
    definition.base.base_id = selected_base.id.as_str().into();
    definition.base.resources[0].pool_id = "recording-generation".into();
    definition.base.resources[0]
        .content
        .as_mut()
        .unwrap()
        .base_id = definition.base.base_id.clone();
    let config = crate::StdHostConfig {
        host_id: definition.authority.host_id.clone(),
        boot_id: definition.authority.boot_id.clone(),
        offer_generation: OfferGeneration(1),
    };

    let (mut enlarged, _) = selected(write);
    enlarged.authority.boot_id = definition.authority.boot_id.clone();
    enlarged.base.resources[0]
        .content
        .as_mut()
        .unwrap()
        .owner_boot = definition.authority.boot_id.clone();
    enlarged.base.base_id = definition.base.base_id.clone();
    enlarged.base.resources[0].pool_id = definition.base.resources[0].pool_id.clone();
    enlarged.base.resources[0].content.as_mut().unwrap().base_id = definition.base.base_id.clone();
    enlarged.retained_bytes += 1;
    assert!(crate::StdHost::from_selected_timer_and_text_file_image(
        config.clone(),
        &image,
        &catalog,
        &checked,
        TextFileInitialization {
            definition: enlarged,
            directory: std::fs::File::open(directory).unwrap(),
            reference: reference.clone(),
        }
    )
    .is_err());
    let expected_grant = definition.authority.clone();
    let host = crate::StdHost::from_selected_timer_and_text_file_image(
        config,
        &image,
        &catalog,
        &checked,
        TextFileInitialization {
            definition,
            directory: std::fs::File::open(directory).unwrap(),
            reference,
        },
    )
    .unwrap();
    assert_eq!(host.text_file_authority_grant(), Some(&expected_grant));
    assert!(host
        .selected_text_file
        .as_ref()
        .unwrap()
        .image_memory
        .is_some());
    if write {
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
    }
    host
}
#[test]
fn cold_image_initializes_real_directory_and_retains_original_operation_grant() {
    let directory = tempfile::tempdir().unwrap();
    drop(cold_host(directory.path(), 64 * 1024 * 1024, true));
}

#[allow(dead_code)] // Reuse only the planning portion of the integration fixture.
#[path = "../../tests/text_file_play/plan.rs"]
mod play_fixture;

#[test]
fn selected_image_original_issuer_publishes_locally_without_global_settlement() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            use crate::body_execution::BodyRunRequest;
            use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
            let root = tempfile::tempdir().unwrap();
            let mut host = cold_host(root.path(), 64 * 1024 * 1024, true);
            let memory = host.kernel_resources.image_memory_observer();
            let baseline = memory.load(std::sync::atomic::Ordering::SeqCst);
            let local = play_fixture::plan(&host, true);
            let remote_root = tempfile::tempdir().unwrap();
            let remote = crate::StdHost::new_with_text_file(
                crate::StdHostConfig {
                    host_id: "remote/member".into(),
                    boot_id: "boot/remote".into(),
                    offer_generation: OfferGeneration(1),
                },
                std::fs::File::open(remote_root.path()).unwrap(),
                play_fixture::reference(),
                true,
            )
            .unwrap();
            let remote_plan = play_fixture::named_plan(&remote, "remote/recording");
            let resident = |plan: Plan| BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            };
            let parts = vec![resident(local), resident(remote_plan)];
            let body = Body::born(
                parts[0].plot.source_document_id.clone(),
                parts[0].plot.checked_plot_id.clone(),
                1,
                "birth/test".into(),
            )
            .unwrap()
            .admit_plot(parts[1].plot.clone(), "admit/test".into())
            .unwrap();
            let wake = body.wake(1, "wake/test".into()).unwrap().1;
            let plan = BodyPlan::seal(&wake, parts).unwrap();
            let issued = host.prospective_body_play(&wake, &plan).unwrap();
            let control = crate::RunControl::default();
            let request = || BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            };
            struct Output(usize);
            impl crate::body_execution::BodyForeOutputAdapter for Output {
                fn deliver(&mut self, _: &crate::ExternalForeDelivery) -> Result<(), String> {
                    self.0 += 1;
                    Ok(())
                }
            }
            let input = [crate::ExternalForeInput {
                front_port_id: port_id("value"),
                track: ConnectionTrack::Payload,
                bytes: b"I was here.".to_vec(),
            }];
            let mut output = Output(0);
            let pressure = host
                .kernel_resources
                .reserve_managed_base_memory(64 * 1024 * 1024 - baseline - 1)
                .unwrap();
            assert!(host
                .run_selected_image_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &input,
                    &mut output,
                    &mut Vec::new(),
                    &mut crate::ThreadTimer,
                    |_, _| panic!("memory refusal must precede start"),
                )
                .is_err());
            assert_eq!(host.prospective_body_play(&wake, &plan).unwrap(), issued);
            assert_eq!(output.0, 0);
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
            drop(pressure);
            assert_eq!(memory.load(std::sync::atomic::Ordering::SeqCst), baseline);
            let mut starts = 0;
            let report = host
                .run_selected_image_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &input,
                    &mut output,
                    &mut Vec::new(),
                    &mut crate::ThreadTimer,
                    |play, _| {
                        assert_eq!(play, &issued);
                        starts += 1;
                        Ok(())
                    },
                )
                .unwrap();
            let local = report.local_execution();
            assert_eq!(local.local_terminal(), &TerminalDisposition::Completed);
            assert_eq!(local.local_execution().wake_at_start, wake);
            assert_eq!(local.local_partitions().len(), 1);
            assert!(local.failure().is_none());
            assert_eq!(starts, 1);
            assert!(memory.load(std::sync::atomic::Ordering::SeqCst) > baseline);
            let saved_reference = local.fore_deliveries()[0].bytes.to_vec();
            drop(report);
            assert_eq!(memory.load(std::sync::atomic::Ordering::SeqCst), baseline);
            assert_eq!(output.0, 1);
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
            assert_eq!(std::fs::read_dir(remote_root.path()).unwrap().count(), 0);
            assert!(host
                .run_selected_image_issuing_host_plan_with_fore_to_with_start(
                    request(),
                    &issued,
                    &input,
                    &mut output,
                    &mut Vec::new(),
                    &mut crate::ThreadTimer,
                    |_, _| panic!("replay must refuse before start"),
                )
                .is_err());
            assert_eq!(output.0, 1);
            drop(host);
            let mut reader = cold_host(root.path(), 64 * 1024 * 1024, false);
            let read_plan = play_fixture::plan(&reader, false);
            let read_body = Body::born(
                read_plan.source_document_id.clone(),
                read_plan.checked_plot_id.clone(),
                1,
                "birth/read".into(),
            )
            .unwrap();
            let read_wake = read_body.wake(1, "wake/read".into()).unwrap().1;
            let read_plan = BodyPlan::seal(&read_wake, vec![resident(read_plan)]).unwrap();
            let read_play = reader
                .prospective_body_play(&read_wake, &read_plan)
                .unwrap();
            let read = reader
                .run_selected_image_issuing_host_plan_with_fore_to_with_start(
                    BodyRunRequest {
                        wake: &read_wake,
                        plan: &read_plan,
                        control: &control,
                        keyboard: None,
                    },
                    &read_play,
                    &[crate::ExternalForeInput {
                        front_port_id: port_id("selected"),
                        track: ConnectionTrack::Payload,
                        bytes: saved_reference,
                    }],
                    &mut output,
                    &mut Vec::new(),
                    &mut crate::ThreadTimer,
                    |_, _| Ok(()),
                )
                .unwrap();
            assert_eq!(
                reader.advertisement().boot_id.as_str(),
                "boot/selected-file-reader"
            );
            assert_eq!(
                read.local_execution().local_terminal(),
                &TerminalDisposition::Completed
            );
            assert_eq!(
                read.local_execution().fore_deliveries()[0].bytes.as_slice(),
                b"I was here."
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
