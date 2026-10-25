//! Private pinned metadata reconstruction, with no corpus/resource publication.
use conduit_core::*;
use conduit_language::{train_lexicon::*, *};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub fn pinned_owner() -> PreparedTrainLexicon {
    let raw = std::fs::read(
        std::env::var_os("CONDUIT_EWT_TRAIN_CONLLU").expect("explicit existing pinned TRAIN path"),
    )
    .unwrap();
    let hex: String = semantic_digest("language/ewt-train-conllu@1", &raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        hex,
        "661219e208cf02bcc7cffbd9635605cf0778081e8839a92c48da4e73ad39558d"
    );
    let mut support: BTreeMap<&str, BTreeSet<u8>> = BTreeMap::new();
    for line in std::str::from_utf8(&raw).unwrap().lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 10
            || fields[0].is_empty()
            || !fields[0].bytes().all(|b| b.is_ascii_digit())
        {
            continue;
        }
        let code = POS_CODES
            .iter()
            .position(|p| *p == fields[3])
            .expect("TRAIN UPOS") as u8;
        support.entry(fields[1]).or_default().insert(code);
    }
    let rows: Vec<_> = support
        .into_iter()
        .filter(|(surface, codes)| !surface.is_empty() && surface.len() <= 128 && codes.len() <= 4)
        .collect();
    assert_eq!(rows.len(), ENTRIES);
    let mut bytes = b"CLXPOS1\0".to_vec();
    bytes
        .extend((0..32).map(|i| u8::from_str_radix(&SOURCE_SHA256[i * 2..i * 2 + 2], 16).unwrap()));
    bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    for (surface, codes) in rows {
        bytes.extend_from_slice(&(surface.len() as u16).to_le_bytes());
        bytes.extend_from_slice(surface.as_bytes());
        bytes.push(codes.len() as u8);
        bytes.extend(codes);
    }
    assert_eq!(bytes.len(), RESOURCE_BYTES);
    assert_eq!(semantic_digest(IDENTITY_DOMAIN, &bytes), CONTENT_DIGEST);
    let reference = pinned_train_lexicon_reference();
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("private-resource-snapshot-fixture"),
        authority_contract: AuthorityContractId::from(READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("private-resource-snapshot-fixture"),
        maximum_bytes: RESOURCE_BYTES as u64,
        maximum_items: Some(ENTRIES as u64),
        availability: ResourceReferenceAvailability::Available,
    };
    PreparedTrainLexicon::admit(reference, &binding, Arc::from(bytes))
        .unwrap_or_else(|e| panic!("private adoption: {e:?}"))
}
pub fn source(text: &str, finality: LanguageTextFinality) -> LanguageTextRevision {
    LanguageTextRevision::new(
        finality,
        LanguageText::new(
            LanguageTextId::new("opaque-snapshot-fixture".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("1".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        LinguisticDerivationProvenance::deterministic_rule(
            "snapshot-unit-fixture".into(),
            "1".into(),
        )
        .unwrap(),
        0,
        Some(text.chars().count() as u32),
    )
    .unwrap()
}
