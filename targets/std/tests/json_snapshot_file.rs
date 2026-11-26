#![cfg(target_os = "linux")]
//! Actual selected immutable files; no installed recorder/ACK or sandbox claim.
use conduit_core::*;
use conduit_linux_snapshot_store::{
    PreparedFileSnapshot, SnapshotDirectory, SnapshotRecordSelection,
};
use conduit_std_host::json_snapshot_file::{
    JsonSnapshotFileRefusal as E, PreparedJsonSnapshot, SelectedJsonSnapshotResidence,
};
use std::fs::File;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn binding(boot: &str, write: bool) -> (PlannedGear, BoundedResourceRef) {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("text/json-utf8@1"),
        access_class: conduit_std_offers::JSON_SNAPSHOT_ACCESS_CLASS.into(),
        extent: ResourceExtent {
            bytes: 11,
            items: Some(1),
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
    let offer = conduit_std_offers::json_snapshot_offer(content.clone()).unwrap();
    let host: HostId = "notebook-storage".into();
    let boot: BootId = boot.into();
    let authority = AuthorityBinding {
        grant_id: format!(
            "grant/{}/{}",
            boot.as_str(),
            if write { "save" } else { "load" }
        )
        .into(),
        contract_id: conduit_std_offers::JSON_SNAPSHOT_AUTHORITY.into(),
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
            base_id: conduit_std_offers::JSON_SNAPSHOT_BASE.into(),
            residence_profile: kind_id(conduit_std_offers::JSON_SNAPSHOT_RESIDENCE),
        }),
    };
    (
        planned_gear_from_parts! {
            semantic_contract: offer.semantic_contract,
            placement_id: "publication".into(), gear_id: "publication".into(),
            kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![ConfigurationEntry {key: "reference".into(), value: ConfigurationValue::Text(hex(&reference.encode().unwrap()))}], host_id: host, boot_id: boot, offer_generation: OfferGeneration(1),
            capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id, base: None,
            realization_characteristics: vec![], limits: offer.limits,
            inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: vec![],
            host_calls: offer.host_calls, resources: vec![resource, ResourceBinding {pool_id: "json/preparation".into(), class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(), units: conduit_std_offers::JSON_SNAPSHOT_PREPARATION_BYTES, protected: None,compute:None,content:None}], authority: vec![authority], pool_references: vec![],
        },
        reference,
    )
}

// Trusted fixture owns this directory and independently authorizes one operation.
// Neither the PlannedGear nor incoming reference supplies this original grant.
fn selection(root: &std::path::Path, boot: &str, write: bool) -> SelectedJsonSnapshotResidence {
    let reference = binding(boot, write).1;
    let original = AuthorityGrant {
        grant_id: format!("grant/{boot}/{}", if write { "save" } else { "load" }).into(),
        contract_id: conduit_std_offers::JSON_SNAPSHOT_AUTHORITY.into(),
        host_call_contract_id: if write {
            conduit_std_offers::JSON_SNAPSHOT_PUBLISH_CALL
        } else {
            conduit_std_offers::JSON_SNAPSHOT_READ_CALL
        }
        .into(),
        subject_kind: if write {
            "resource/publish-json-snapshot"
        } else {
            "resource/read-json-snapshot"
        }
        .into(),
        host_id: "notebook-storage".into(),
        boot_id: boot.into(),
        capability_id: if write {
            conduit_std_offers::JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION
        } else {
            conduit_std_offers::JSON_SNAPSHOT_READ_IMPLEMENTATION
        }
        .into(),
    };
    let local = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "fixture/actual-directory".into(),
        authority_contract: original.contract_id.clone(),
        authority_grant: original.grant_id.clone(),
        maximum_bytes: 256,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    SelectedJsonSnapshotResidence::prepare(File::open(root).unwrap(), reference, original, local)
        .unwrap()
}
#[test]
fn exact_json_reference_reopens_under_new_boot_without_text_conversion() {
    let root = tempfile::tempdir().unwrap();
    let (write, reference) = binding("boot/one", true);
    let selected = selection(root.path(), "boot/one", true);
    let mut writer = selected.prepare_operation(&write).unwrap();
    let encoded = writer
        .publish(&write.authority[0], br#"{"value":1}"#)
        .unwrap()
        .to_vec();
    assert_eq!(encoded, reference.encode().unwrap());
    assert_eq!(BoundedResourceRef::decode(&encoded).unwrap(), reference);
    let (read, _) = binding("boot/two", false);
    let selected_read = selection(root.path(), "boot/two", false);
    let mut reader = PreparedJsonSnapshot::prepare(&selected_read, &read).unwrap();
    assert_eq!(
        reader.read(&read.authority[0], &encoded).unwrap(),
        br#"{"value":1}"#
    );
    let mut wrong = encoded.clone();
    wrong.push(0);
    assert!(matches!(
        reader.read(&read.authority[0], &wrong),
        Err(E::WrongReference)
    ));
    reader.retire();
    assert!(matches!(
        reader.read(&read.authority[0], &encoded),
        Err(E::Unavailable)
    ));
    writer.retire();
    assert!(matches!(
        writer.publish(&write.authority[0], br#"{"value":1}"#),
        Err(E::Unavailable)
    ));
}
#[test]
fn malformed_content_foreign_authority_and_occupied_generation_never_replace_content() {
    let root = tempfile::tempdir().unwrap();
    let (write, reference) = binding("boot/current", true);
    let selected = selection(root.path(), "boot/current", true);
    let mut writer = selected.prepare_operation(&write).unwrap();
    for bad in [b"not JSON!!!".as_slice(), b"{invalid:1}", br#"{"value":}"#] {
        assert!(matches!(
            writer.publish(&write.authority[0], bad),
            Err(E::InvalidJson)
        ));
    }
    let mut stale = write.authority[0].clone();
    stale.boot_id = "boot/stale".into();
    assert!(writer.publish(&stale, br#"{"value":1}"#).is_err());
    writer
        .publish(&write.authority[0], br#"{"value":1}"#)
        .unwrap();
    let mut occupied = selected.prepare_operation(&write).unwrap();
    assert!(occupied
        .publish(&write.authority[0], br#"{"other":2}"#)
        .is_err());
    let (read, _) = binding("boot/new", false);
    let selected_read = selection(root.path(), "boot/new", false);
    let mut reader = selected_read.prepare_operation(&read).unwrap();
    assert_eq!(
        reader
            .read(&read.authority[0], &reference.encode().unwrap())
            .unwrap(),
        br#"{"value":1}"#
    );
}
#[test]
fn original_authority_and_exact_placement_configuration_are_independent() {
    let root = tempfile::tempdir().unwrap();
    let (write, reference) = binding("boot/current", true);
    let selected = selection(root.path(), "boot/current", true);
    let mut wrong_profile = reference.clone();
    wrong_profile.content_profile = "value/text".into();
    assert!(matches!(
        SelectedJsonSnapshotResidence::prepare(
            File::open(root.path()).unwrap(),
            wrong_profile,
            selected.authority().clone(),
            selected.local_reference().clone()
        ),
        Err(E::WrongReference)
    ));
    let mut wrong_local = selected.local_reference().clone();
    wrong_local.version = ResourceVersionIdentity::from_digest([8; 32]);
    let mismatched_local = SelectedJsonSnapshotResidence::prepare(
        File::open(root.path()).unwrap(),
        reference.clone(),
        selected.authority().clone(),
        wrong_local,
    )
    .unwrap();
    assert!(PreparedJsonSnapshot::prepare(&mismatched_local, &write).is_err());
    let mut reversed = write.clone();
    reversed.resources.reverse();
    assert!(PreparedJsonSnapshot::prepare(&selected, &reversed).is_ok());
    let foreign = selection(root.path(), "boot/foreign", true);
    assert!(PreparedJsonSnapshot::prepare(&foreign, &write).is_err());
    let mut wrong = write.clone();
    wrong.configuration.clear();
    assert!(matches!(
        PreparedJsonSnapshot::prepare(&selected, &wrong),
        Err(E::InvalidBinding)
    ));
    let mut wrong = write.clone();
    wrong.host_calls[0].maximum_input_bytes -= 1;
    assert!(matches!(
        PreparedJsonSnapshot::prepare(&selected, &wrong),
        Err(E::InvalidBinding)
    ));
    let mut wrong = write.clone();
    wrong.resources[0].content.as_mut().unwrap().base_id = "foreign/base".into();
    assert!(matches!(
        PreparedJsonSnapshot::prepare(&selected, &wrong),
        Err(E::InvalidBinding)
    ));
    let mut wrong = write.clone();
    wrong.resources[0].units = 2;
    assert!(matches!(
        PreparedJsonSnapshot::prepare(&selected, &wrong),
        Err(E::InvalidBinding)
    ));
    let mut wrong = write.clone();
    let mut foreign_reference = reference.clone();
    foreign_reference.lifetime.version = ResourceVersionIdentity::from_digest([9; 32]);
    wrong.configuration[0].value =
        ConfigurationValue::Text(hex(&foreign_reference.encode().unwrap()));
    assert!(matches!(
        PreparedJsonSnapshot::prepare(&selected, &wrong),
        Err(E::WrongReference)
    ));
    let mut wrong = write.clone();
    wrong.authority[0].capability_id = "foreign".into();
    assert!(PreparedJsonSnapshot::prepare(&selected, &wrong).is_err());
}
#[test]
fn restored_opaque_file_must_still_be_valid_json() {
    let root = tempfile::tempdir().unwrap();
    let (write, reference) = binding("boot/current", true);
    let directory = SnapshotDirectory::prepare(File::open(root.path()).unwrap()).unwrap();
    let selected_write = selection(root.path(), "boot/current", true);
    PreparedFileSnapshot::prepare_selected(
        &directory,
        SnapshotRecordSelection {
            host: &write.host_id,
            boot: &write.boot_id,
            kind: &write.kind_id,
            capability: &write.capability_id,
            binding: &write.resources[0],
            host_call: &write.host_calls[0],
            authority: &write.authority[0],
        },
        &reference,
        selected_write.authority(),
        selected_write.local_reference(),
    )
    .unwrap()
    .publish(&write.authority[0], b"not JSON!!!")
    .unwrap();
    let (read, _) = binding("boot/new", false);
    let selected = selection(root.path(), "boot/new", false);
    let mut reader = selected.prepare_operation(&read).unwrap();
    assert!(matches!(
        reader.read(&read.authority[0], &reference.encode().unwrap()),
        Err(E::InvalidJson)
    ));
}
