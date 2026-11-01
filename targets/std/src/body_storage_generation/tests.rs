use super::*;
use crate::{
    body_storage::{BodyStorageBounds, SelectedBodyStorage},
    StdHostConfig,
};
use std::os::unix::fs::PermissionsExt;
fn bounds() -> BodyStorageBounds {
    BodyStorageBounds {
        maximum_snapshot_records: 2,
        maximum_record_bytes: 4096,
        generation_slots: 1,
        publication_slots: 1,
        reader_leases: 1,
        state_slots: 1,
        maximum_state_bytes: 4096,
        total_disk_bytes: 131072,
        preparation_bytes: 1024 * 1024,
    }
}
fn host(root: &Path) -> StdHost {
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/storage-generation".into(),
        boot_id: "boot/storage-generation".into(),
        offer_generation: c::OfferGeneration(1),
    });
    host.attach_selected_body_storage(
        SelectedBodyStorage::open(root, "test/storage", bounds(), None).unwrap(),
    )
    .unwrap();
    host
}
fn reference(version: u8) -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 4096,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([version; 32]),
            expires_at: None,
        },
    }
}
fn request(version: u8, access: c::ResourceAccessMode) -> BodyStorageGenerationRequest {
    BodyStorageGenerationRequest {
        snapshot: Some(BodyStorageSnapshotRequest {
            reference: reference(version),
            access,
        }),
        state: None,
    }
}
fn state_request() -> BodyStorageGenerationRequest {
    let ty = c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID)).unwrap();
    let value =
        c::StructuredInfoValue::leaf(ty.clone(), c::InfoBool::FALSE.encode().to_vec()).unwrap();
    BodyStorageGenerationRequest {
        snapshot: None,
        state: Some(BodyStorageStateRequest {
            binding: crate::state_value::DurableStateBinding {
                body: "body/storage-test".into(),
                state: "state/cell".into(),
                value_kind: c::kind_id(c::BOOL_INFO_ID),
                maximum_value_bytes: 4096,
            },
            type_name: "Cell".into(),
            value_type: ty,
            default_value: value,
        }),
    }
}
#[test]
fn exact_snapshot_generation_refresh_preserves_unrelated_inventory_and_cursors() {
    let root = tempfile::tempdir().unwrap();
    let mut host = host(root.path());
    let before = host.advertisement.clone();
    let sign = host.next_kernel_sign_sequence;
    let play = host.next_kernel_play_sequence;
    let evidence = host
        .select_body_storage_generation(request(2, c::ResourceAccessMode::WriteCandidatePublish))
        .unwrap();
    assert_eq!(evidence.retained_snapshots, 0);
    eprintln!(
        "snapshot storage preparation: {} bytes",
        evidence.preparation_bytes
    );
    assert!(evidence.preparation_bytes <= bounds().preparation_bytes);
    let selected = host.selected_body_storage_generation().unwrap();
    assert_eq!(selected.snapshot_reference(), Some(&reference(2)));
    assert!(selected.state_resource().is_none());
    for resource in &before.resources {
        assert!(host.advertisement.resources.contains(resource));
    }
    for offer in &before.capabilities {
        assert!(host.advertisement.capabilities.contains(offer));
    }
    assert_eq!(host.next_kernel_sign_sequence, sign);
    assert_eq!(host.next_kernel_play_sequence, play);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    // Presence selects only an exact read residence; no record validity/commit
    // claim is made here. The existing backend still must verify the envelope.
    let name = snapshot_name(&host.advertisement.host_id, &reference(2));
    std::fs::write(root.path().join(name), b"bounded but untrusted record").unwrap();
    host.select_body_storage_generation(request(2, c::ResourceAccessMode::ReadPublished))
        .unwrap();
    assert_eq!(
        host.selected_body_storage_generation()
            .unwrap()
            .snapshot_resource()
            .unwrap()
            .content
            .as_ref()
            .unwrap()
            .contract
            .access,
        c::ResourceAccessMode::ReadPublished
    );
    assert_eq!(host.next_kernel_sign_sequence, sign);
    assert_eq!(host.next_kernel_play_sequence, play);
    let previous = host.advertisement.clone();
    let registry = host.base_registry.clone();
    std::fs::write(
        root.path().join(format!(".pending-{}", "a".repeat(64))),
        b"candidate",
    )
    .unwrap();
    assert!(host
        .select_body_storage_generation(request(3, c::ResourceAccessMode::WriteCandidatePublish))
        .unwrap_err()
        .contains("candidate"));
    assert_eq!(host.advertisement, previous);
    assert_eq!(host.base_registry, registry);
}
#[test]
fn state_offer_on_default_host_and_finite_refusals_leave_previous_selection() {
    let root = tempfile::tempdir().unwrap();
    let mut host = host(root.path());
    let evidence = host
        .select_body_storage_generation(state_request())
        .unwrap();
    eprintln!(
        "State storage preparation: {} bytes",
        evidence.preparation_bytes
    );
    assert!(host
        .selected_body_storage_generation()
        .unwrap()
        .state_resource()
        .is_some());
    assert!(host
        .selected_body_storage_generation()
        .unwrap()
        .snapshot_resource()
        .is_none());
    let before = host.advertisement.clone();
    let retained = host.selected_body_storage.clone().unwrap();
    assert!(host
        .select_body_storage_generation(state_request())
        .unwrap_err()
        .contains("holder"));
    assert_eq!(host.advertisement, before);
    drop(retained);
    std::os::unix::fs::symlink("/dev/null", root.path().join("b".repeat(64))).unwrap();
    assert!(host
        .select_body_storage_generation(state_request())
        .is_err());
    assert_eq!(host.advertisement, before);
}
#[test]
fn full_retained_slots_and_storage_preparation_refuse_before_ready() {
    let root = tempfile::tempdir().unwrap();
    let mut host = host(root.path());
    let before = host.advertisement.clone();
    for digit in ["a", "b"] {
        std::fs::write(root.path().join(digit.repeat(64)), b"retained").unwrap();
    }
    assert!(host
        .select_body_storage_generation(request(2, c::ResourceAccessMode::WriteCandidatePublish))
        .unwrap_err()
        .contains("slot"));
    assert_eq!(host.advertisement, before);
    assert!(host.selected_body_storage_generation().is_none());
    let root2 = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root2.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut b = bounds();
    b.preparation_bytes = 4096;
    let mut host = StdHost::new();
    host.attach_selected_body_storage(
        SelectedBodyStorage::open(root2.path(), "test/small", b, None).unwrap(),
    )
    .unwrap();
    let before = host.advertisement.clone();
    assert!(host
        .select_body_storage_generation(state_request())
        .unwrap_err()
        .contains("preparation"));
    assert_eq!(host.advertisement, before);
    assert_eq!(std::fs::read_dir(root2.path()).unwrap().count(), 0);
}

#[test]
fn occupied_state_slot_is_bound_to_exact_authored_body_state_key() {
    let root = tempfile::tempdir().unwrap();
    let mut host = host(root.path());
    let selected = state_request();
    let binding = selected.state.as_ref().unwrap().binding.clone();
    let key = crate::state_value::FileDurableStateResidence::storage_key(&binding).unwrap();
    std::fs::create_dir(root.path().join(&key)).unwrap();
    host.select_body_storage_generation(selected).unwrap();
    assert_eq!(
        host.selected_body_storage_generation()
            .unwrap()
            .state_binding(),
        Some(&binding)
    );
    let before = host.advertisement.clone();
    let mut other = state_request();
    other.state.as_mut().unwrap().binding.body = "body/other".into();
    assert!(host
        .select_body_storage_generation(other)
        .unwrap_err()
        .contains("another exact binding"));
    assert_eq!(host.advertisement, before);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn selected_1024_state_narrows_offer_and_preserves_original_host_inventory() {
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut selected_bounds = bounds();
    selected_bounds.maximum_state_bytes = 1024;
    let mut host = StdHost::new();
    host.attach_selected_body_storage(
        SelectedBodyStorage::open(root.path(), "test/selected-1024", selected_bounds, None)
            .unwrap(),
    )
    .unwrap();
    let before = host.advertisement.clone();
    let old_input = host.advertisement.capabilities[0].inputs.as_ptr();
    let sign = host.next_kernel_sign_sequence;
    let play = host.next_kernel_play_sequence;
    assert!(host
        .select_body_storage_generation(state_request())
        .unwrap_err()
        .contains("configured profile"));
    assert_eq!(host.advertisement, before);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    let mut selected = state_request();
    selected.state.as_mut().unwrap().binding.maximum_value_bytes = 1024;
    host.select_body_storage_generation(selected).unwrap();
    let entry = host.selected_body_storage_generation().unwrap();
    assert_eq!(entry.state_binding().unwrap().maximum_value_bytes, 1024);
    let offer = &entry.base.capabilities[0];
    assert_eq!(offer.limits.max_queue_bytes, 1024);
    assert_eq!(offer.host_calls[0].maximum_input_bytes, 1024);
    assert_eq!(offer.host_calls[1].maximum_output_bytes, 1024);
    for old in &before.capabilities {
        assert!(host.advertisement.capabilities.contains(old));
    }
    for old in &before.resources {
        assert!(host.advertisement.resources.contains(old));
    }
    assert_eq!(
        host.advertisement
            .capabilities
            .iter()
            .find(|c| c.capability_id == before.capabilities[0].capability_id)
            .unwrap()
            .inputs
            .as_ptr(),
        old_input
    );
    assert_eq!(host.next_kernel_sign_sequence, sign);
    assert_eq!(host.next_kernel_play_sequence, play);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
