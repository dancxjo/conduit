//! CPU checkpoint preservation with an independently signed protocol fixture.
//! This creates no admitted member or erase authority and exercises no hardware.
use super::*;
fn plain(
    evidence: &BodyBiographyEvidence,
    segments: &[BodyBiographyArchiveSegment],
    manager: &AdmissionManager,
    cursor: &network::Cursor,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    stream_with_admissions_and_network(
        evidence,
        segments,
        &[],
        Some(manager),
        Some(cursor),
        &mut |part: &[u8]| {
            bytes.extend_from_slice(part);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    bytes
}
#[test]
fn network_cursor_production_serializers_preserve_pending_and_clean_and_refuse_tampering() {
    let (segments, evidence) = fixture();
    let key = SigningKey::from_bytes(&[63; 32]);
    let body = &evidence.body_id;
    let root = key.verifying_key().to_bytes();
    let manager = AdmissionManager::new(body.clone()).unwrap();
    for phase in [1, 2] {
        let cursor = network::fixture(body, &key, phase);
        let bytes = plain(&evidence, &segments, &manager, &cursor);
        let decoded = decode_resident_with_prefix(reader(&bytes), body, &root, None).unwrap();
        assert!(decoded.network.as_ref() == Some(&cursor));
        assert_eq!(decoded.retained.schema, SCHEMA_V4);
        let mut output = Vec::new();
        stream_rewrite_with_network(
            &evidence,
            &[],
            &manager,
            &decoded.summary,
            decoded.span,
            None,
            decoded.local_count,
            &root,
            &decoded.layouts,
            decoded.network.as_ref(),
            reader(&bytes),
            &mut |part: &[u8]| {
                output.extend_from_slice(part);
                Ok::<_, ()>(())
            },
        )
        .unwrap();
        let rewritten = decode_resident_with_prefix(reader(&output), body, &root, None).unwrap();
        assert!(rewritten.network.as_ref() == Some(&cursor));
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for mutation in 0..6 {
            let mut value = original.clone();
            match mutation {
                0 => {
                    value.as_object_mut().unwrap().remove("network_cursor");
                }
                1 => value["schema"] = serde_json::json!(SCHEMA_V3),
                2 => value["schema"] = serde_json::json!("conduit.environment/root-restart@5"),
                3 => value["network_cursor"] = serde_json::json!("00"),
                4 => {
                    value["network_cursor"] = serde_json::json!(value["network_cursor"]
                        .as_str()
                        .unwrap()
                        .to_ascii_uppercase())
                }
                _ => {
                    let mut hex = value["network_cursor"]
                        .as_str()
                        .unwrap()
                        .as_bytes()
                        .to_vec();
                    hex[100] = if hex[100] == b'0' { b'1' } else { b'0' };
                    value["network_cursor"] = serde_json::json!(String::from_utf8(hex).unwrap());
                }
            }
            let bad = serde_json::to_vec(&value).unwrap();
            assert!(decode_resident_with_prefix(reader(&bad), body, &root, None).is_err());
        }
        assert!(decode_resident_with_prefix(
            reader(&bytes),
            body,
            &SigningKey::from_bytes(&[64; 32]).verifying_key().to_bytes(),
            None
        )
        .is_err());
        assert!(archive::decode(&mut reader(&bytes), body, None).is_err());
    }
}
