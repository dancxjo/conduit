//! Catalogs select immutable references; they do not acknowledge publications.
use super::*;
use conduit_core as c;
fn references() -> Vec<c::BoundedResourceRef> {
    (1..=15)
        .map(|version| c::BoundedResourceRef {
            identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: "text/json-utf8@1".into(),
            access_class: "resource/snapshot@1".into(),
            extent: c::ResourceExtent {
                bytes: 3448,
                items: Some(1),
            },
            lifetime: c::ResourceLifetime {
                version: c::ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        })
        .collect()
}
#[test]
fn overnight_catalog_reserves_its_own_record_and_restores_exact_order_without_allocations() {
    let references = references();
    let catalog = PreparedRecordingCatalog::prepare(&references, 16, 1_048_576).unwrap();
    assert_eq!(catalog.required_records(), 16);
    assert_eq!(
        catalog.required_content_bytes(),
        15 * 3448 + catalog.content().len() as u64
    );
    assert!(catalog.content().len() <= 4096);
    let mut reader = PreparedRecordingCatalogReader::default();
    let (result, memory) = allocation_probe::observe(|| reader.read(catalog.content()));
    assert_eq!(result, Ok(()));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(reader.len(), 15);
    for (index, reference) in references.iter().enumerate() {
        assert_eq!(
            reader.encoded_reference(index),
            Some(reference.encode().unwrap().as_slice())
        );
    }
    assert!(reader.encoded_reference(15).is_none());
    assert_eq!(
        PreparedRecordingCatalog::prepare(&references, 15, 1_048_576).err(),
        Some(RecordingCatalogRefusal::Capacity)
    );
    assert_eq!(
        PreparedRecordingCatalog::prepare(&references, 16, catalog.required_content_bytes() - 1)
            .err(),
        Some(RecordingCatalogRefusal::Capacity)
    );
}
#[test]
fn malformed_duplicate_and_incompatible_catalogs_preserve_loaded_references() {
    let references = references();
    let catalog = PreparedRecordingCatalog::prepare(&references, 16, 1_048_576).unwrap();
    let mut reader = PreparedRecordingCatalogReader::default();
    reader.read(catalog.content()).unwrap();
    let first = reader.encoded_reference(0).unwrap().to_vec();
    let mut invalid = vec![
        "{}".to_string(),
        catalog.content().replace("references", "foreign"),
        catalog.content().replace('a', "A"),
        format!("{}x", catalog.content()),
    ];
    let duplicate = catalog.content().replace(
        &references[1]
            .encode()
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        &references[0]
            .encode()
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    );
    invalid.push(duplicate);
    for content in invalid {
        let (result, memory) = allocation_probe::observe(|| reader.read(&content));
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(reader.len(), 15);
        assert_eq!(reader.encoded_reference(0), Some(first.as_slice()));
    }
    let mut foreign = references.clone();
    foreign[0].content_profile = "foreign".into();
    assert!(PreparedRecordingCatalog::prepare(&foreign, 16, 1_048_576).is_err());
    let mut too_many = references.clone();
    too_many.extend(references.iter().take(2).cloned());
    assert_eq!(
        PreparedRecordingCatalog::prepare(&too_many, 64, 1_048_576).err(),
        Some(RecordingCatalogRefusal::Capacity)
    );
}
