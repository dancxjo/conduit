//! Exercise selected Host custody rather than supplying ambient residence paths.
#![cfg(all(feature = "linux-snapshot", target_os = "linux"))]
use super::*;
use conduit_std_host::body_storage::{BodyStorageBounds, SelectedBodyStorage};
use conduit_std_host::body_storage_generation::{
    BodyStorageGenerationRequest, BodyStorageSnapshotRequest, BodyStorageStateRequest,
};
use std::os::unix::fs::PermissionsExt;

#[test]
fn selected_descriptor_state_executes_and_recovers_with_original_body_admission() {
    thread(|| {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut root_identity = None;
        let mut body_id = None;
        for (boot, input) in [
            ("selected/first", b"original reading".as_slice()),
            ("selected/second", b"next reading".as_slice()),
        ] {
            let mut host = StdHost::new_with_config(StdHostConfig {
                host_id: "selected/storage-host".into(),
                boot_id: boot.into(),
                offer_generation: OfferGeneration(1),
            });
            let selected = SelectedBodyStorage::open(
                root.path(),
                "selected/body-storage",
                BodyStorageBounds {
                    maximum_snapshot_records: 2,
                    maximum_record_bytes: 4096,
                    generation_slots: 2,
                    publication_slots: 1,
                    reader_leases: 1,
                    state_slots: 1,
                    maximum_state_bytes: 4096,
                    total_disk_bytes: 131072,
                    preparation_bytes: 1024 * 1024,
                },
                root_identity.as_ref(),
            )
            .unwrap();
            root_identity = Some(selected.identity().clone());
            host.attach_selected_body_storage(selected).unwrap();
            let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
            let (_, source, _) = checked_source();
            let intended_body = conduit_body::Body::born(
                source.expanded.source_document_id.clone(),
                source.expanded.checked_plot_id.clone(),
                1,
                "sign/born".into(),
            )
            .unwrap();
            let state_gear = source
                .expanded
                .gears
                .iter()
                .find(|gear| gear.kind_id.as_str() == STATE_VALUE_KIND)
                .unwrap();
            host.select_body_storage_generation(BodyStorageGenerationRequest {
                snapshot: Some(BodyStorageSnapshotRequest {
                    reference: BoundedResourceRef {
                        identity: ResourceSemanticIdentity::from_digest([71; 32]),
                        content_profile: "text/json-utf8@1".into(),
                        access_class: "resource/snapshot@1".into(),
                        extent: ResourceExtent {
                            bytes: 64,
                            items: Some(1),
                        },
                        lifetime: ResourceLifetime {
                            version: ResourceVersionIdentity::from_digest([72; 32]),
                            expires_at: None,
                        },
                    },
                    access: ResourceAccessMode::WriteCandidatePublish,
                }),
                state: Some(BodyStorageStateRequest {
                    binding: conduit_std_host::state_value::DurableStateBinding {
                        body: intended_body.body_id.as_str().into(),
                        state: state_gear.gear_id.as_str().into(),
                        value_kind: kind_id(TEXT_INFO_ID),
                        maximum_value_bytes: 4096,
                    },
                    type_name: "Text".into(),
                    value_type: ty.clone(),
                    default_value: StructuredInfoValue::leaf(ty, Vec::new()).unwrap(),
                }),
            })
            .unwrap();
            // Advertising a snapshot does not require selecting or granting it
            // for this State-only Plan.
            assert!(host
                .selected_body_storage_generation()
                .unwrap()
                .snapshot_resource()
                .is_some());
            let (mut host, plan) = planned_on(host);
            let (wake, body) = body_plan(plan);
            if let Some(id) = &body_id {
                assert_eq!(id, &body.body_id);
            } else {
                body_id = Some(body.body_id.clone());
            }
            let inputs = [ExternalForeInput {
                front_port_id: port_id("write"),
                track: ConnectionTrack::Payload,
                bytes: input.to_vec(),
            }];
            let mut capture = BorrowedCapture(Vec::with_capacity(2));
            if boot == "selected/first" {
                let (foreign_wake, foreign_body) =
                    checkpoint_fixture::body_plan_at_birth(body.plots[0].plan.clone(), 2);
                let foreign = host.run_body_plan_with_selected_storage_borrowed_to_with_start(
                    BodyRunRequest {
                        wake: &foreign_wake,
                        plan: &foreign_body,
                        control: &RunControl::default(),
                        keyboard: None,
                    },
                    &inputs,
                    &mut capture,
                    &[],
                    None,
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |_, _| panic!("another Body cannot acquire this State residence"),
                );
                assert!(foreign
                    .unwrap_err()
                    .contains("exact selected storage binding"));
                assert!(capture.0.is_empty());
                assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
                let refused = host.run_body_plan_with_selected_storage_borrowed_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body,
                        control: &RunControl::default(),
                        keyboard: None,
                    },
                    &inputs,
                    &mut capture,
                    &[],
                    None,
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |_, _| Err("start not retained".into()),
                );
                assert!(refused.is_err());
                assert!(capture.0.is_empty());
                assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
            }
            let report = host
                .run_body_plan_with_selected_storage_borrowed_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body,
                        control: &RunControl::default(),
                        keyboard: None,
                    },
                    &inputs,
                    &mut capture,
                    &[],
                    None,
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |play, _| {
                        assert!(play.validate_for(&body));
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(
                report.terminal,
                TerminalDisposition::Completed,
                "{:?}",
                report.failure
            );
            assert!(report.failure.is_none());
            assert!(report.cleanup_failure.is_none());
            if boot == "selected/second" {
                assert_eq!(capture.0.first().unwrap(), b"original reading");
            }
        }
    });
}
