#![cfg(target_os = "linux")]
//! Actual filesystem boundary; not an installed editor or hostile-code sandbox.
use conduit_core::*;
use conduit_linux_snapshot_store::SnapshotDirectory;
use conduit_std_host::text_file::{PreparedTextFile, TextFileRefusal};
use std::fs::{self, File};

fn binding(boot: &str, write: bool) -> (PlannedGear, BoundedResourceRef) {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("value/text"),
        access_class: conduit_data::DATA_GENERATION_ACCESS_CLASS.into(),
        extent: ResourceExtent {
            bytes: 11,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let content = ResourceContentRequirement {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        maximum_bytes: 256,
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
    let offer = conduit_std_offers::durable_text_offer(content.clone()).unwrap();
    let host: HostId = "notebook-storage".into();
    let boot: BootId = boot.into();
    let authority = AuthorityBinding {
        grant_id: format!(
            "grant/{}/{}",
            boot.as_str(),
            if write { "save" } else { "load" }
        )
        .into(),
        contract_id: conduit_std_offers::DURABLE_TEXT_AUTHORITY.into(),
        host_call_contract_id: offer.host_calls[0].contract_id.clone(),
        subject_kind: offer.kind_id.clone(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let resource = ResourceBinding {
        pool_id: "notebook-selected-root".into(),
        class_id: reference.access_class.clone(),
        units: 1,
        protected: None,
        compute: None,
        content: Some(ResourceContentOffer {
            contract: content,
            owner_host: host.clone(),
            owner_boot: boot.clone(),
            base_id: conduit_std_offers::DURABLE_TEXT_BASE.into(),
            residence_profile: kind_id(conduit_std_offers::DURABLE_TEXT_RESIDENCE),
        }),
    };
    (
        planned_gear_from_parts! {
            semantic_contract: offer.semantic_contract,
            placement_id: "publication".into(), gear_id: "publication".into(),
            kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![], host_id: host, boot_id: boot, offer_generation: OfferGeneration(1),
            capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id, base: None,
            realization_characteristics: vec![], limits: offer.limits,
            inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: vec![],
            host_calls: offer.host_calls, resources: vec![resource], authority: vec![authority], pool_references: vec![],
        },
        reference,
    )
}

#[test]
fn explicit_text_publication_reopens_the_exact_data_generation_after_new_boot() {
    let root = tempfile::tempdir().unwrap();
    let (write, reference) = binding("boot/one", true);
    let encoded = {
        let directory = SnapshotDirectory::prepare(File::open(root.path()).unwrap()).unwrap();
        let mut writer = PreparedTextFile::prepare(&directory, &write, &reference).unwrap();
        writer
            .publish(&write.authority[0], b"I was here.")
            .unwrap()
            .to_vec()
    };
    assert_eq!(
        conduit_data::DataReference::decode_for(&kind_id("value/text"), &encoded)
            .unwrap()
            .reference(),
        &reference
    );
    let (read, _) = binding("boot/two", false);
    let directory = SnapshotDirectory::prepare(File::open(root.path()).unwrap()).unwrap();
    let mut reader = PreparedTextFile::prepare(&directory, &read, &reference).unwrap();
    assert_eq!(
        reader.load(&read.authority[0], &encoded).unwrap(),
        b"I was here."
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    reader.retire();
    assert!(matches!(
        reader.load(&read.authority[0], &encoded),
        Err(TextFileRefusal::Unavailable)
    ));
}

#[test]
fn refused_publication_preserves_the_draft_and_never_returns_a_saved_reference() {
    use conduit_semantic_catalog::{BoundedTextState, TextDocument};
    let draft = BoundedTextState::from_document(
        256,
        TextDocument {
            revision: 19,
            text: b"I was here.",
        },
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let directory = SnapshotDirectory::prepare(File::open(root.path()).unwrap()).unwrap();
    let (write, reference) = binding("boot/current", true);
    let mut writer = PreparedTextFile::prepare(&directory, &write, &reference).unwrap();
    let mut stale = write.authority[0].clone();
    stale.boot_id = "boot/old".into();
    assert!(writer.publish(&stale, draft.current()).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    assert_eq!(
        (draft.revision(), draft.current()),
        (19, b"I was here.".as_slice())
    );
    writer.retire();
    assert!(matches!(
        writer.publish(&write.authority[0], draft.current()),
        Err(TextFileRefusal::Unavailable)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn wrong_reference_invalid_text_and_conflicting_publication_do_not_replace_saved_bytes() {
    let root = tempfile::tempdir().unwrap();
    let directory = SnapshotDirectory::prepare(File::open(root.path()).unwrap()).unwrap();
    let (write, reference) = binding("boot/current", true);
    let mut writer = PreparedTextFile::prepare(&directory, &write, &reference).unwrap();
    assert!(matches!(
        writer.publish(&write.authority[0], &[0xff; 11]),
        Err(TextFileRefusal::InvalidText)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let encoded = writer
        .publish(&write.authority[0], b"I was here.")
        .unwrap()
        .to_vec();
    let mut conflict = PreparedTextFile::prepare(&directory, &write, &reference).unwrap();
    assert!(conflict
        .publish(&write.authority[0], b"other draft")
        .is_err());
    let (read, _) = binding("boot/current", false);
    let mut reader = PreparedTextFile::prepare(&directory, &read, &reference).unwrap();
    let mut wrong = encoded.clone();
    wrong[1] ^= 1;
    assert!(matches!(
        reader.load(&read.authority[0], &wrong),
        Err(TextFileRefusal::WrongReference)
    ));
    assert_eq!(
        reader.load(&read.authority[0], &encoded).unwrap(),
        b"I was here."
    );
}

#[test]
fn idle_offer_transition_preserves_boot_and_refuses_another_document_or_root() {
    use conduit_std_host::{StdHost, StdHostConfig};
    let root = tempfile::tempdir().unwrap();
    let other_root = tempfile::tempdir().unwrap();
    let (_, reference) = binding("boot/current", true);
    let mut host = StdHost::new_with_text_file(
        StdHostConfig {
            host_id: "notebook-storage".into(),
            boot_id: "boot/current".into(),
            offer_generation: OfferGeneration(1),
        },
        File::open(root.path()).unwrap(),
        reference.clone(),
        true,
    )
    .unwrap();
    let original = host.advertisement().clone();
    host.transition_text_file_offer(File::open(root.path()).unwrap(), reference.clone(), false)
        .unwrap();
    let read = host.advertisement().clone();
    assert_eq!(read.host_id, original.host_id);
    assert_eq!(read.boot_id, original.boot_id);
    assert_eq!(read.offer_generation.0, original.offer_generation.0 + 1);
    assert!(read
        .capabilities
        .iter()
        .any(|offer| offer.implementation.implementation_id.as_str()
            == conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION));
    assert!(!read
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == conduit_data::DATA_SAVE_TEXT_KIND));
    let mut next = reference.clone();
    next.lifetime.version = ResourceVersionIdentity::from_digest([3; 32]);
    host.transition_text_file_offer(File::open(root.path()).unwrap(), next.clone(), true)
        .unwrap();
    let write = host.advertisement().clone();
    assert_eq!(write.boot_id, original.boot_id);
    assert_eq!(write.offer_generation.0, original.offer_generation.0 + 2);
    assert!(host
        .transition_text_file_offer(File::open(other_root.path()).unwrap(), next.clone(), false,)
        .is_err());
    assert_eq!(host.advertisement(), &write);
    next.identity = ResourceSemanticIdentity::from_digest([9; 32]);
    assert!(host
        .transition_text_file_offer(File::open(root.path()).unwrap(), next, false,)
        .is_err());
    assert_eq!(host.advertisement(), &write);
    assert_eq!(
        original
            .capabilities
            .iter()
            .filter(|offer| ![
                conduit_data::DATA_SAVE_TEXT_KIND,
                conduit_data::DATA_LOAD_TEXT_KIND
            ]
            .contains(&offer.kind_id.as_str()))
            .collect::<Vec<_>>(),
        write
            .capabilities
            .iter()
            .filter(|offer| ![
                conduit_data::DATA_SAVE_TEXT_KIND,
                conduit_data::DATA_LOAD_TEXT_KIND
            ]
            .contains(&offer.kind_id.as_str()))
            .collect::<Vec<_>>()
    );
}
