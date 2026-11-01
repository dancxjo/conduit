//! Selected descriptor-backed snapshot execution; no recorder ACK claim.
use super::*;
use conduit_std_host::body_storage::{BodyStorageBounds, SelectedBodyStorage};
use conduit_std_host::body_storage_generation::{
    BodyStorageGenerationRequest, BodyStorageSnapshotRequest,
};
use std::os::unix::fs::PermissionsExt;
struct BorrowedCapture(Vec<Vec<u8>>);
impl BodyBorrowedForeOutputAdapter for BorrowedCapture {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if !self.0.is_empty() {
            return Err("one finite output allowed".into());
        }
        self.0.push(value.bytes.to_vec());
        Ok(())
    }
}
#[test]
fn selected_snapshot_requires_authority_and_recovers_identical_bytes_after_host_boot() {
    thread(|| {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
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
        let mut identity = None;
        for write in [true, false] {
            let mut host = StdHost::new_with_config(conduit_std_host::StdHostConfig {
                host_id: "selected/snapshot-host".into(),
                boot_id: if write {
                    "selected/write-boot"
                } else {
                    "selected/read-boot"
                }
                .into(),
                offer_generation: OfferGeneration(1),
            });
            let selected = SelectedBodyStorage::open(
                root.path(),
                "selected/snapshot",
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
                identity.as_ref(),
            )
            .unwrap();
            identity = Some(selected.identity().clone());
            host.attach_selected_body_storage(selected).unwrap();
            host.select_body_storage_generation(BodyStorageGenerationRequest {
                snapshot: Some(BodyStorageSnapshotRequest {
                    reference: reference.clone(),
                    access: if write {
                        ResourceAccessMode::WriteCandidatePublish
                    } else {
                        ResourceAccessMode::ReadPublished
                    },
                }),
                state: None,
            })
            .unwrap();
            let resource = host
                .selected_body_storage_generation()
                .unwrap()
                .snapshot_resource()
                .unwrap();
            let offer = conduit_std_host::linux_snapshot::offer_for_resource(resource).unwrap();
            let requirement = &offer.authority_requirements[0];
            let grant = AuthorityGrant {
                grant_id: "selected/snapshot-grant".into(),
                contract_id: requirement.contract_id.clone(),
                host_call_contract_id: requirement.host_call_contract_id.clone(),
                subject_kind: requirement.subject_kind.clone(),
                host_id: host.advertisement().host_id.clone(),
                boot_id: host.advertisement().boot_id.clone(),
                capability_id: offer.capability_id,
            };
            let (mut host, plan) = planned_on(host, &reference, &grant, write);
            let (wake, body) = body_plan(plan);
            let values = inputs(
                if write {
                    CONTENT.to_vec()
                } else {
                    reference.encode().unwrap()
                }
                .as_slice(),
            );
            let mut capture = BorrowedCapture(Vec::with_capacity(1));
            if write {
                for authority in [
                    None,
                    Some({
                        let mut foreign = grant.clone();
                        foreign.boot_id = "foreign/boot".into();
                        foreign
                    }),
                ] {
                    let result = host.run_body_plan_with_selected_storage_borrowed_to_with_start(
                        BodyRunRequest {
                            wake: &wake,
                            plan: &body,
                            control: &RunControl::default(),
                            keyboard: None,
                        },
                        &values,
                        &mut capture,
                        &[],
                        authority.as_ref(),
                        &mut Vec::new(),
                        &mut ThreadTimer,
                        |_, _| panic!("unselected authority cannot reach accepted start"),
                    );
                    assert!(result.is_err());
                    assert!(capture.0.is_empty());
                    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
                }
            }
            let report = host
                .run_body_plan_with_selected_storage_borrowed_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body,
                        control: &RunControl::default(),
                        keyboard: None,
                    },
                    &values,
                    &mut capture,
                    &[],
                    Some(&grant),
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
            assert_eq!(capture.0.len(), 1);
            if write {
                assert_eq!(capture.0[0], reference.encode().unwrap());
            } else {
                assert_eq!(capture.0[0], CONTENT);
            }
        }
    });
}
