#![cfg(target_has_atomic = "ptr")]
//! Resource/read admission only. No inference, independent eligibility, quality
//! or Source snapshot/epoch integration claim. No corpus/resource output files.
use conduit_core::*;
use conduit_language::train_lexicon::*;
use std::sync::Arc;
fn binding(reference: &BoundedResourceRef) -> ResourceReferenceBinding {
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("local-test-train-lexicon"),
        authority_contract: AuthorityContractId::from(READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("local-test-read-grant"),
        maximum_bytes: RESOURCE_BYTES as u64,
        maximum_items: Some(ENTRIES as u64),
        availability: ResourceReferenceAvailability::Available,
    }
}
#[test]
fn exact_pin_is_not_an_access_grant_or_an_arbitrary_resource_profile() {
    let reference = pinned_train_lexicon_reference();
    assert_eq!(reference.extent.bytes, 222999);
    assert_eq!(reference.extent.items, Some(19652));
    let valid = binding(&reference);
    let mut absent = valid.clone();
    absent.availability = ResourceReferenceAvailability::Lost;
    assert!(matches!(
        PreparedTrainLexicon::admit(reference.clone(), &absent, Arc::from([])),
        Err(TrainLexiconRefusal::Authority(
            ResourceReferenceAccessRefusal::ResourceLost
        ))
    ));
    let mut stale = valid.clone();
    stale.version = ResourceVersionIdentity::from_digest([1; 32]);
    assert!(matches!(
        PreparedTrainLexicon::admit(reference.clone(), &stale, Arc::from([])),
        Err(TrainLexiconRefusal::Authority(
            ResourceReferenceAccessRefusal::StaleVersion
        ))
    ));
    let mut small = valid.clone();
    small.maximum_bytes -= 1;
    assert!(matches!(
        PreparedTrainLexicon::admit(reference.clone(), &small, Arc::from([])),
        Err(TrainLexiconRefusal::Authority(
            ResourceReferenceAccessRefusal::ByteBoundExceeded
        ))
    ));
    assert!(matches!(
        PreparedTrainLexicon::admit(
            reference.clone(),
            &valid,
            Arc::from(vec![0; RESOURCE_BYTES])
        ),
        Err(TrainLexiconRefusal::Content)
    ));
    let mut changed = reference.clone();
    changed.extent.items = Some(1);
    assert!(matches!(
        PreparedTrainLexicon::admit(changed, &valid, Arc::from([])),
        Err(TrainLexiconRefusal::Reference)
    ));
    let mut changed = reference;
    changed.content_profile = "foreign/lexicon@1".into();
    assert!(matches!(
        PreparedTrainLexicon::admit(changed, &valid, Arc::from([])),
        Err(TrainLexiconRefusal::Reference)
    ));
}
fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
#[ignore = "requires explicit existing pinned private EWT TRAIN path; writes no corpus/resource"]
fn private_pinned_train_reproduces_resource_and_all_complete_lookup_rows() {
    use std::collections::{BTreeMap, BTreeSet};
    let path =
        std::env::var_os("CONDUIT_EWT_TRAIN_CONLLU").expect("explicit existing local TRAIN file");
    let raw = std::fs::read(path).unwrap();
    assert_eq!(
        hex(semantic_digest("language/ewt-train-conllu@1", &raw)),
        "661219e208cf02bcc7cffbd9635605cf0778081e8839a92c48da4e73ad39558d"
    );
    let source = std::str::from_utf8(&raw).unwrap();
    let mut support: BTreeMap<&str, BTreeSet<u8>> = BTreeMap::new();
    let mut tokens = 0;
    for line in source.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 10
            || fields[0].is_empty()
            || !fields[0].bytes().all(|b| b.is_ascii_digit())
        {
            continue;
        }
        let code = POS_CODES
            .iter()
            .position(|pos| *pos == fields[3])
            .expect("supported TRAIN UPOS") as u8;
        support.entry(fields[1]).or_default().insert(code);
        tokens += 1;
    }
    assert_eq!(tokens, 204578);
    assert_eq!(support.len(), 19674);
    let entries: Vec<_> = support
        .into_iter()
        .filter(|(surface, codes)| !surface.is_empty() && surface.len() <= 128 && codes.len() <= 4)
        .collect();
    assert_eq!(entries.len(), ENTRIES);
    let mut bytes = b"CLXPOS1\0".to_vec();
    let digest: Vec<u8> = (0..32)
        .map(|i| u8::from_str_radix(&SOURCE_SHA256[i * 2..i * 2 + 2], 16).unwrap())
        .collect();
    bytes.extend_from_slice(&digest);
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (surface, codes) in &entries {
        bytes.extend_from_slice(&(surface.len() as u16).to_le_bytes());
        bytes.extend_from_slice(surface.as_bytes());
        bytes.push(codes.len() as u8);
        bytes.extend(codes);
    }
    assert_eq!(bytes.len(), RESOURCE_BYTES);
    assert_eq!(semantic_digest(IDENTITY_DOMAIN, &bytes), CONTENT_DIGEST);
    let reference = pinned_train_lexicon_reference();
    let owner =
        PreparedTrainLexicon::admit(reference.clone(), &binding(&reference), Arc::from(bytes))
            .unwrap_or_else(|e| panic!("resource admission: {e:?}"));
    assert_eq!(owner.logical_storage_bytes(), 301611);
    for (ordinal, (surface, codes)) in entries.iter().enumerate() {
        let row = owner
            .lookup(surface)
            .unwrap()
            .expect("every complete row present");
        assert!(std::ptr::eq(row.owner(), &owner));
        assert_eq!(row.ordinal(), ordinal);
        assert_eq!(row.surface(), *surface);
        assert_eq!(row.pos_codes(), codes.iter().copied().collect::<Vec<_>>());
        assert!(row.comparisons() <= MAXIMUM_LOOKUP_COMPARISONS);
    }
    assert!(owner
        .lookup("opaque-resource-absence-sentinel-0d13b64a")
        .unwrap()
        .is_none());
    assert!(matches!(
        owner.lookup(""),
        Err(TrainLexiconRefusal::QueryBound)
    ));
    assert!(matches!(
        owner.lookup(&"x".repeat(129)),
        Err(TrainLexiconRefusal::QueryBound)
    ));
}
