use super::*;
pub(crate) fn receipt(body: &conduit_body::BodyId, seed: &[u8; 32], region: u8) -> layout::Receipt {
    use crate::native_observed_archive::custody_journal::{DATA, TOTAL};
    let key = ed25519_dalek::SigningKey::from_bytes(seed);
    let mut bytes = [0; 152];
    bytes[..8].copy_from_slice(b"OLAYOUT1");
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&(DATA as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&(TOTAL as u32).to_le_bytes());
    bytes[24..56].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
    bytes[56..88].fill(region);
    let mut h = Sha256::new();
    h.update(b"conduit.environment/observation-layout@1\0");
    h.update(key.verifying_key().as_bytes());
    h.update(&bytes[..88]);
    bytes[88..].copy_from_slice(&key.sign(&<[u8; 32]>::from(h.finalize())).to_bytes());
    layout::Receipt::decode(bytes)
}
#[test]
fn five_receipts_survive_replay_checkpoint_and_park_with_exact_original_metadata() {
    let (mut store, mut owner, host, key) = super::super::park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let evidence = owner.as_ref().unwrap().evidence().clone();
    let before = serde_json::to_vec(&store.admissions).unwrap();
    store.evidence = Some(evidence.clone());
    store.ready = false;
    for i in 1..=5 {
        let r = receipt(&evidence.body_id, &[7; 32], i);
        let returned = persist(&mut store, r.clone()).unwrap();
        assert_eq!(returned.bytes(), r.bytes());
        let generation = store.journal.generation();
        assert!(persist(&mut store, r).is_err());
        assert_eq!(store.journal.generation(), generation);
    }
    let generation = store.journal.generation();
    assert!(persist(&mut store, receipt(&evidence.body_id, &[7; 32], 6)).is_err());
    assert!(persist(&mut store, receipt(&evidence.body_id, &[8; 32], 7)).is_err());
    assert!(persist(
        &mut store,
        receipt(
            &serde_json::from_value(serde_json::json!("body/foreign")).unwrap(),
            &[7; 32],
            7
        )
    )
    .is_err());
    assert_eq!(store.journal.generation(), generation);
    let mut seed = PrivateSeed([0; 32]);
    let (decoded, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &evidence.body_id,
        &mut seed.0,
        store.custody.as_ref(),
    )
    .unwrap();
    assert_eq!(decoded.retained.schema, payload::SCHEMA_V3);
    assert_eq!(decoded.retained.evidence, evidence);
    assert_eq!(
        serde_json::to_vec(&decoded.retained.admissions).unwrap(),
        before
    );
    for i in 1..=5 {
        assert!(decoded.layouts.find(&[i; 32]).is_some());
    }
    drop(decoded);
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let mut parked =
        park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap();
    let restored =
        restore_store(&mut store, &mut parked, &host.host_id, &host.boot_id, &key).unwrap();
    assert_eq!(restored.evidence(), &evidence);
    for i in 1..=5 {
        assert!(store
            .custody
            .as_ref()
            .unwrap()
            .layouts
            .find(&[i; 32])
            .is_some());
    }
}
#[test]
fn layout_decode_refuses_wrong_scope_signature_schema_duplicates_and_extent() {
    let body: conduit_body::BodyId =
        serde_json::from_value(serde_json::json!("body/layout")).unwrap();
    let r = receipt(&body, &[7; 32], 1);
    let mut list = payload::layout::Layouts::default();
    list.add(&r).unwrap();
    let json = serde_json::to_vec(&list).unwrap();
    let parsed: payload::layout::Layouts = serde_json::from_slice(&json).unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).verifying_key();
    parsed.validate(&body, &key).unwrap();
    assert!(parsed
        .validate(
            &serde_json::from_value(serde_json::json!("body/foreign")).unwrap(),
            &key
        )
        .is_err());
    assert!(parsed
        .validate(
            &body,
            &ed25519_dalek::SigningKey::from_bytes(&[8; 32]).verifying_key()
        )
        .is_err());
    let raw: serde_json::Value = serde_json::from_slice(&json).unwrap();
    let one = raw[0].as_str().unwrap();
    assert!(
        serde_json::from_value::<payload::layout::Layouts>(serde_json::json!([one, one])).is_err()
    );
    assert!(
        serde_json::from_value::<payload::layout::Layouts>(serde_json::json!([&one[..302]]))
            .is_err()
    );
    let mut bad = one.as_bytes().to_vec();
    bad[0] = b'F';
    assert!(
        serde_json::from_value::<payload::layout::Layouts>(serde_json::json!([
            core::str::from_utf8(&bad).unwrap()
        ]))
        .is_err()
    );
    let mut corrupt = *r.bytes();
    corrupt[88] ^= 1;
    let mut list = payload::layout::Layouts::default();
    list.add(&layout::Receipt::decode(corrupt)).unwrap();
    assert!(list.validate(&body, &key).is_err());
}
#[test]
fn restart_v3_requires_receipt_field_and_pinned_root_without_weakening_legacy() {
    let (store, owner, _host, key) = super::super::park_tests::fixture();
    let evidence = owner.as_ref().unwrap().evidence();
    let mut plain = Vec::new();
    payload::stream_with_admissions(
        evidence,
        &store.archives,
        &[],
        store.admissions.as_ref(),
        &mut |b| {
            plain.extend_from_slice(b);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    let legacy: serde_json::Value = serde_json::from_slice(&plain).unwrap();
    let mut layouts = payload::layout::Layouts::default();
    layouts
        .add(&receipt(&evidence.body_id, &[7; 32], 1))
        .unwrap();
    let mut v3 = legacy.clone();
    v3["schema"] = serde_json::json!(payload::SCHEMA_V3);
    v3["observation_layouts"] = serde_json::to_value(&layouts).unwrap();
    let decode = |value: &serde_json::Value, root: &[u8; 32]| {
        let bytes = serde_json::to_vec(value).unwrap();
        let mut at = 0;
        payload::decode_resident_with_prefix(
            |out| {
                let n = out.len().min(bytes.len() - at);
                out[..n].copy_from_slice(&bytes[at..at + n]);
                at += n;
                Ok(n)
            },
            &evidence.body_id,
            root,
            None,
        )
    };
    assert!(decode(&legacy, &key).is_ok());
    assert!(decode(&v3, &key).is_ok());
    assert!(decode(
        &v3,
        &ed25519_dalek::SigningKey::from_bytes(&[8; 32])
            .verifying_key()
            .to_bytes()
    )
    .is_err());
    for schema in [payload::SCHEMA, payload::SCHEMA_V2, "unknown"] {
        let mut bad = v3.clone();
        bad["schema"] = serde_json::json!(schema);
        assert!(decode(&bad, &key).is_err());
    }
    let mut missing = v3.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("observation_layouts");
    assert!(decode(&missing, &key).is_err());
    // No resident may escape before EOF and the shallower crypto boundary.
    let bytes = serde_json::to_vec(&v3).unwrap();
    let read = |bytes: Vec<u8>| {
        let mut at = 0;
        move |out: &mut [u8; 128]| {
            let n = out.len().min(bytes.len() - at);
            out[..n].copy_from_slice(&bytes[at..at + n]);
            at += n;
            Ok(n)
        }
    };
    assert!(payload::decode_resident(read(bytes.clone()), &evidence.body_id, None).is_err());
    let mut empty = v3.clone();
    empty["observation_layouts"] = serde_json::json!([]);
    assert!(payload::decode_resident(
        read(serde_json::to_vec(&empty).unwrap()),
        &evidence.body_id,
        None
    )
    .is_err());
    for malformed in [
        bytes[..bytes.len() - 1].to_vec(),
        [bytes.as_slice(), b" trailing"].concat(),
    ] {
        assert!(payload::decode_resident_with_prefix(
            read(malformed.clone()),
            &evidence.body_id,
            &key,
            None
        )
        .is_err());
        assert!(payload::decode_resident(read(malformed), &evidence.body_id, None).is_err());
    }
    let foreign = conduit_body::Body::born(
        "foreign-source".into(),
        "foreign-plot".into(),
        1,
        "foreign".into(),
    )
    .unwrap()
    .body_id;
    let mut wrong_body = v3.clone();
    let mut foreign_layouts = payload::layout::Layouts::default();
    foreign_layouts
        .add(&receipt(&foreign, &[7; 32], 1))
        .unwrap();
    wrong_body["observation_layouts"] = serde_json::to_value(foreign_layouts).unwrap();
    assert!(decode(&wrong_body, &key).is_err());
    let mut bad_signature = v3.clone();
    let mut hex = bad_signature["observation_layouts"][0]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec();
    let last = hex.len() - 1;
    hex[last] = if hex[last] == b'0' { b'1' } else { b'0' };
    bad_signature["observation_layouts"][0] = serde_json::json!(String::from_utf8(hex).unwrap());
    assert!(decode(&bad_signature, &key).is_err());
    let mut bad = v3;
    bad["observation_layouts"][0] = serde_json::json!("00");
    assert!(decode(&bad, &key).is_err());
}
#[test]
fn cold_signing_key_refuses_unavailable_or_foreign_root() {
    let (mut store, _owner, _host, key) = super::super::park_tests::fixture();
    assert!(signing_key(&store, &key).is_err()); // active lifecycle owner
    store.ready = false;
    assert!(signing_key(&store, &key).is_ok());
    assert!(signing_key(&store, &[0; 32]).is_err());
    store.parked = true;
    assert!(signing_key(&store, &key).is_err());
    store.parked = false;
    store.failed = true;
    assert!(signing_key(&store, &key).is_err());
}
#[test]
fn layout_write_refusal_returns_no_durable_receipt_and_preserves_retry_source() {
    let (mut store, mut owner, _host, key) = super::super::park_tests::fixture();
    let region = crate::native_evidence_region::test_empty();
    let media = region.media.clone();
    store.journal = Checkpoint::open(region).unwrap();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let evidence = owner.as_ref().unwrap().evidence().clone();
    store.evidence = Some(evidence.clone());
    store.ready = false;
    let generation = store.journal.generation();
    let source = *store.custody.as_ref().unwrap().source.digest();
    let receipt = receipt(&evidence.body_id, &[7; 32], 1);
    media.lock().unwrap().1 = false;
    assert!(persist(&mut store, receipt.clone()).is_err());
    assert_eq!(store.journal.generation(), generation);
    assert_eq!(*store.custody.as_ref().unwrap().source.digest(), source);
    assert!(store.custody.as_ref().unwrap().layouts.is_empty());
    media.lock().unwrap().1 = true;
    assert_eq!(
        persist(&mut store, receipt.clone()).unwrap().bytes(),
        receipt.bytes()
    );
}

#[test]
fn observation_layout_rewrite_preserves_authenticated_network_cursor() {
    let (mut store, mut owner, _host, key, expected) = super::super::park_tests::network_fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let original = owner.as_ref().unwrap().evidence().clone();
    store.evidence = Some(original.clone());
    store.ready = false;
    let layout = receipt(&original.body_id, &[7; 32], 1);
    persist(&mut store, layout.clone()).unwrap();
    assert!(store.custody.as_ref().unwrap().network.as_ref() == Some(&expected));
    let mut seed = PrivateSeed([0; 32]);
    let (decoded, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &original.body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    assert!(decoded.network.as_ref() == Some(&expected));
    assert_eq!(decoded.retained.schema, payload::SCHEMA_V4);
    assert_eq!(
        decoded.layouts.find(&[1; 32]).unwrap().bytes(),
        layout.bytes()
    );
}
