use super::super::publication::tests::{finish_worker, fixture, independent_read_grant};
use super::*;
#[test]
fn source_catalog_without_private_readback_witness_refuses_before_mutation() {
    let (root, disk, mut owner, publish) = fixture();
    let generation = owner.host.advertisement().offer_generation;
    let resident = owner.resident.clone();
    assert!(owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .verified_reference()
        .is_none());
    let error = owner
        .prepare_source_recording_catalog(&root, &publish, 1024 * 1024)
        .err()
        .unwrap();
    assert!(error.contains("actual matching admitted readback"));
    assert_eq!(owner.host.advertisement().offer_generation, generation);
    assert_eq!(owner.resident, resident);
    assert!(owner.session.realization().is_none());
    assert_eq!(owner.source_recording_stage.as_ref().unwrap().len(), 1);
    assert_eq!(std::fs::read_dir(&disk).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn source_catalog_worker_uses_verified_chunk_and_original_body_state() {
    let (root, disk, mut owner, publish) = fixture();
    let body = owner.session.evidence().body_id.clone();
    let boot = owner.host.advertisement().boot_id.clone();
    let original = owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .observations()
        .next()
        .unwrap()
        .canonical()
        .to_vec();
    let chunk = owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .reference
        .clone();
    let worker = owner
        .start_service_source_publication(&root, 5000, &publish, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert!(owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .verified_reference()
        .is_none());
    let read = independent_read_grant(&mut owner);
    let worker = owner
        .start_service_source_readback(&root, 5000, &read, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert_eq!(
        owner
            .source_recording_stage
            .as_ref()
            .unwrap()
            .verified_reference(),
        Some(&chunk)
    );
    // The fixture explicitly authorizes catalog publication with its retained
    // publish capability; readback witness/reference alone issues no authority.
    let worker = owner
        .start_service_source_catalog(&root, 5000, &publish, 1024 * 1024)
        .unwrap();
    let selected = owner.host.advertisement().clone();
    finish_worker(&root, &mut owner, worker);
    assert_eq!(owner.host.advertisement(), &selected);
    assert_eq!(owner.session.evidence().body_id, body);
    assert_eq!(owner.host.advertisement().boot_id, boot);
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    let stage = owner.source_recording_stage.as_ref().unwrap();
    assert_eq!(stage.len(), 1);
    assert_eq!(stage.observations().next().unwrap().canonical(), original);
    assert_eq!(stage.verified_reference(), Some(&chunk));
    let receipt = owner.last_execution.as_ref().unwrap();
    assert_eq!(
        receipt["source_catalog"]["catalog_checkpoint_observed"], true,
        "{receipt}"
    );
    assert_eq!(receipt["source_catalog"]["matching_current_fore"], true);
    assert_eq!(
        receipt["source_catalog"]["producer_retirement_authorized"],
        false
    );
    let binding = owner
        .host
        .selected_body_storage_generation()
        .unwrap()
        .state_binding()
        .unwrap();
    assert_eq!(binding.body, body.as_str());
    assert_eq!(binding.maximum_value_bytes, 1024);
    assert_eq!(std::fs::read_dir(&disk).unwrap().count(), 3);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn catalog_seed_binding_is_stable_across_snapshot_references_and_read_source() {
    let (root, _, owner, _) = fixture();
    let reference = &owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .reference;
    let mut other = reference.clone();
    other.lifetime.version = c::ResourceVersionIdentity::from_digest([9; 32]);
    let mut bindings = Vec::new();
    for reference in [reference, &other] {
        let encoded = reference.encode().unwrap();
        let hex: String = encoded.iter().map(|b| format!("{b:02x}")).collect();
        let text = include_str!(
            "../../../../../../../bodies/environment/profiles/catalog-checkpoint.conduit"
        )
        .replace("@REFERENCE@", &hex);
        let plot = crate::plot_source::parse(&text)
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
        bindings.push(
            seed::extract(&plot, owner.session.evidence().body_id.as_str())
                .unwrap()
                .request
                .binding,
        );
    }
    let read = crate::plot_source::parse(include_str!(
        "../../../../../../../bodies/environment/profiles/catalog-seed-read.conduit"
    ))
    .unwrap()
    .expand_entry_for_authoring()
    .unwrap();
    let recovered = seed::extract(&read, owner.session.evidence().body_id.as_str())
        .unwrap()
        .request
        .binding;
    assert_eq!(bindings[0], bindings[1]);
    assert_eq!(bindings[0], recovered);
    std::fs::remove_dir_all(root).unwrap();
}
