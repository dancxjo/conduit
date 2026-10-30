use super::*;
use conduit_body::{self as b, BodyId};
use conduit_core as c;
use ed25519_dalek::{Signer, SigningKey};
fn fixture() -> (Vec<BodyBiographyArchiveSegment>, BodyBiographyEvidence) {
    let workset = b::BodyWorkset::from_plots([b::ResidentPlot::new(
        "source/test".into(),
        "checked/test".into(),
    )])
    .unwrap();
    let body = b::Body::born_with_plots(workset, 1, "sign/birth".into()).unwrap();
    let membership = b::BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence =
        b::BodyBiographyEvidence::born(body, membership, "Custody Fixture".into()).unwrap();
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/custody".into(),
        boot_id: "boot/0".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/custody".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut body =
        conduit_environment_runtime::body_image::open_installed_image(evidence, &host).unwrap();
    let mut archives = Vec::new();
    for i in 1..=120 {
        body = b::BodyLifecycleSession::resume_here(
            body.evidence().clone(),
            &host.host_id,
            &format!("boot/{i}").into(),
        )
        .unwrap();
        if !body.pending_archives().is_empty() {
            archives.extend_from_slice(body.pending_archives());
            let head = body.pending_archives().last().unwrap().digest;
            body.acknowledge_archives(head).unwrap();
            if archives.len() >= 2 {
                return (archives, body.evidence().clone());
            }
        }
    }
    panic!("fixture failed to cross the actual membership compaction boundary")
}

fn reader(bytes: &[u8]) -> impl FnMut(&mut [u8; 128]) -> Result<usize, ()> + '_ {
    let mut at = 0;
    move |out| {
        let n = 7.min(bytes.len() - at);
        out[..n].copy_from_slice(&bytes[at..at + n]);
        at += n;
        Ok(n)
    }
}
fn receipt(
    body: &BodyId,
    segments: &[BodyBiographyArchiveSegment],
    key: &SigningKey,
) -> wire::Receipt {
    let mut summary = wire::Summary::default();
    for segment in segments {
        summary.append(body, segment).unwrap();
    }
    let mut result = wire::Receipt {
        body: body.clone(),
        summary,
        offer: wire::Hex([3; 32]),
        consumer: wire::Consumer::Owner {
            key: wire::Hex([4; 32]),
        },
        acknowledged: wire::Hex([5; 32]),
        signature: wire::Hex([0; 64]),
    };
    result.signature = wire::Hex(
        key.sign(&result.transcript(&key.verifying_key().to_bytes()))
            .to_bytes(),
    );
    result
}
fn plain_v2(
    evidence: &BodyBiographyEvidence,
    prefix: &wire::Receipt,
    suffix: &[BodyBiographyArchiveSegment],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"{\"schema\":");
    json::stream(&SCHEMA_V2, &mut |b: &[u8]| {
        out.extend_from_slice(b);
        Ok(())
    })
    .unwrap();
    out.extend_from_slice(b",\"evidence\":");
    json::stream(&profile::EvidenceRef::from(evidence), &mut |b: &[u8]| {
        out.extend_from_slice(b);
        Ok(())
    })
    .unwrap();
    out.extend_from_slice(b",\"biography_prefix\":");
    json::stream(prefix, &mut |b: &[u8]| {
        out.extend_from_slice(b);
        Ok(())
    })
    .unwrap();
    out.extend_from_slice(b",\"archives\":");
    json::stream(&Archives(suffix, &[]), &mut |b: &[u8]| {
        out.extend_from_slice(b);
        Ok(())
    })
    .unwrap();
    out.extend_from_slice(b",\"admissions\":null}");
    out
}
#[test]
fn prefix_rewrite_drops_only_acknowledged_local_bytes_and_reopens_offline() {
    let (segments, evidence) = fixture();
    let body = &evidence.body_id;
    let manager = AdmissionManager::new(body.clone()).unwrap();
    let key = SigningKey::from_bytes(&[61; 32]);
    let root = key.verifying_key().to_bytes();
    let mut original = Vec::new();
    stream_compressed_with_admissions(
        &evidence,
        &segments,
        &[],
        Some(&manager),
        &mut |b: &[u8]| {
            original.extend_from_slice(b);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    let prior = decode_resident(reader(&original), body, None).unwrap();
    assert_eq!(prior.local_count, 2);
    assert!(prior.prefix.is_none());
    let prefix = receipt(body, &segments, &key);
    let mut migrated = Vec::new();
    let (summary, span) = stream_rewrite_with_prefix(
        &evidence,
        &[],
        &manager,
        &prior.summary,
        prior.span,
        Some(&prefix),
        0,
        &root,
        reader(&original),
        &mut |b: &[u8]| {
            migrated.extend_from_slice(b);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert!(migrated.len() < original.len());
    let reopened = decode_resident_with_prefix(reader(&migrated), body, &root, None).unwrap();
    assert_eq!(reopened.local_count, 0);
    assert_eq!(reopened.summary, summary);
    assert_eq!(reopened.span, span);
    assert_eq!(reopened.retained.evidence, evidence);
    assert_eq!(reopened.retained.admissions, Some(manager.clone()));
    assert_eq!(
        reopened.prefix.as_ref().unwrap().signature,
        prefix.signature
    );
    assert!(decode_resident(reader(&migrated), body, None).is_err());
    let custody = archive::Custody {
        summary: &summary,
        prefix: Some(&prefix),
        local_count: 0,
    };
    decode_resident_with_prefix(reader(&migrated), body, &root, Some(custody)).unwrap();
    // Another ordinary checkpoint preserves prefix and empty suffix without a comma.
    let mut next = Vec::new();
    stream_rewrite_with_prefix(
        &evidence,
        &[],
        &manager,
        &summary,
        span,
        Some(&prefix),
        0,
        &root,
        reader(&migrated),
        &mut |b: &[u8]| {
            next.extend_from_slice(b);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!(
        decode_resident_with_prefix(reader(&next), body, &root, None)
            .unwrap()
            .local_count,
        0
    );
}
#[test]
fn prefix_decoder_validates_real_suffix_counts_keys_and_hold_policy() {
    let (segments, evidence) = fixture();
    let body = &evidence.body_id;
    let key = SigningKey::from_bytes(&[62; 32]);
    let root = key.verifying_key().to_bytes();
    let prefix = receipt(body, &segments[..1], &key);
    let bytes = plain_v2(&evidence, &prefix, &segments[1..]);
    let resident = decode_resident_with_prefix(reader(&bytes), body, &root, None).unwrap();
    assert_eq!(resident.summary.count, 2);
    assert_eq!(resident.local_count, 1);
    assert_eq!(resident_extent(reader(&bytes)).unwrap().segments, 1);
    let foreign = SigningKey::from_bytes(&[63; 32]).verifying_key().to_bytes();
    assert!(decode_resident_with_prefix(reader(&bytes), body, &foreign, None).is_err());
    let mut corrupt = prefix.clone();
    corrupt.signature.0[0] ^= 1;
    assert!(decode_resident_with_prefix(
        reader(&plain_v2(&evidence, &corrupt, &segments[1..])),
        body,
        &root,
        None
    )
    .is_err());
    let missing = plain_v2(&evidence, &prefix, &[]);
    assert!(decode_resident_with_prefix(reader(&missing), body, &root, None).is_err());
    let bad_custody = archive::Custody {
        summary: &resident.summary,
        prefix: Some(&prefix),
        local_count: 0,
    };
    assert!(decode_resident_with_prefix(reader(&bytes), body, &root, Some(bad_custody)).is_err());
    // Native policy is the same NoHold archive decoder used by the suffix.
    // A syntactically complete local Wake/Plan with null HOLD parses; changing
    // only HOLD to an object refuses before importing any Core Plan.
    let mut archive = serde_json::to_value(&segments[1]).unwrap();
    archive["wakes"] = serde_json::json!([{"wake_id":"wake/policy","body_id":body,"workset":evidence.body.workset,"workload_revision":1,"wake_sequence":1,"lifecycle":"Lulled","plans":[{"plan_id":"plan/policy","active_play_id":null,"state":"Superseded","hold":null}],"sign_ids":[],"events":[]}]);
    let valid = serde_json::to_vec(&archive).unwrap();
    assert!(
        profile::deserialize_archive(&mut serde_json::Deserializer::from_slice(&valid)).is_ok()
    );
    archive["wakes"][0]["plans"][0]["hold"] = serde_json::json!({});
    let forbidden = serde_json::to_vec(&archive).unwrap();
    assert!(
        profile::deserialize_archive(&mut serde_json::Deserializer::from_slice(&forbidden))
            .is_err()
    );
    for cut in [0, 1, bytes.len() / 2, bytes.len() - 1] {
        assert!(decode_resident_with_prefix(reader(&bytes[..cut]), body, &root, None).is_err());
    }
    let mut trailing = bytes;
    trailing.push(b'x');
    assert!(decode_resident_with_prefix(reader(&trailing), body, &root, None).is_err());
}

#[cfg(any(feature = "observation-store", feature = "local-evidence-store"))]
#[test]
fn valid_layout_cannot_hide_forged_prefix_at_post_eof_receipt_boundary() {
    let (segments, evidence) = fixture();
    let body = &evidence.body_id;
    let seed = [61; 32];
    let key = SigningKey::from_bytes(&seed);
    let root = key.verifying_key().to_bytes();
    let prefix = receipt(body, &segments, &key);
    let mut layouts = layout::Layouts::default();
    layouts
        .add(&super::super::observed::tests::receipt(body, &seed, 1))
        .unwrap();
    let encode = |prefix: &wire::Receipt| {
        let mut value: serde_json::Value =
            serde_json::from_slice(&plain_v2(&evidence, prefix, &[])).unwrap();
        value["schema"] = serde_json::json!(SCHEMA_V3);
        value["observation_layouts"] = serde_json::to_value(&layouts).unwrap();
        serde_json::to_vec(&value).unwrap()
    };
    // Prefix ordering is part of the stream contract; preserve it when adding @3.
    let encode = |prefix: &wire::Receipt| {
        let bytes = encode(prefix);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        format!("{{\"schema\":{},\"evidence\":{},\"biography_prefix\":{},\"archives\":[],\"admissions\":null,\"observation_layouts\":{}}}", value["schema"], value["evidence"], value["biography_prefix"], value["observation_layouts"]).into_bytes()
    };
    let valid = encode(&prefix);
    let (resident, ordinary_memory) = crate::allocation_probe::observe(|| {
        decode_resident_with_prefix(reader(&valid), body, &root, None).unwrap()
    });
    let (boxed, boxed_memory) = crate::allocation_probe::observe(|| {
        decode_resident_boxed_with_prefix(reader(&valid), body, &root, None).unwrap()
    });
    assert_eq!(boxed_memory.allocations, ordinary_memory.allocations);
    assert_eq!(
        boxed_memory.retained_bytes(),
        ordinary_memory.retained_bytes() + core::mem::size_of::<archive::Resident>()
    );
    println!(
        "boxed decode final backing={} allocations={} peak={} retained={}",
        core::mem::size_of::<archive::Resident>(),
        boxed_memory.allocations,
        boxed_memory.peak_bytes,
        boxed_memory.retained_bytes()
    );
    assert_eq!(boxed.summary, resident.summary);
    assert!(boxed.layouts == resident.layouts);
    assert_eq!(
        serde_json::to_vec(&boxed.prefix).unwrap(),
        serde_json::to_vec(&resident.prefix).unwrap()
    );
    assert_eq!(boxed.retained.evidence, resident.retained.evidence);
    for cut in [0, 1, valid.len() / 2, valid.len() - 1] {
        assert!(
            decode_resident_boxed_with_prefix(reader(&valid[..cut]), body, &root, None).is_err()
        );
    }
    let mut trailing = valid.clone();
    trailing.push(b'x');
    assert!(decode_resident_boxed_with_prefix(reader(&trailing), body, &root, None).is_err());
    assert!(decode_resident_boxed_with_prefix(
        reader(&valid),
        body,
        &SigningKey::from_bytes(&[62; 32]).verifying_key().to_bytes(),
        None
    )
    .is_err());
    assert!(decode_resident_boxed_with_prefix(
        reader(&valid),
        &serde_json::from_str::<BodyId>("\"body/foreign\"").unwrap(),
        &root,
        None
    )
    .is_err());
    let mut forged = prefix.clone();
    forged.signature.0[0] ^= 1;
    let bytes = encode(&forged);
    assert!(decode_resident_with_prefix(reader(&bytes), body, &root, None).is_err());
    assert!(decode_resident_boxed_with_prefix(reader(&bytes), body, &root, None).is_err());
    // Even an exact retained transcript comparison cannot replace crypto.
    let custody = archive::Custody {
        summary: &resident.summary,
        prefix: Some(&forged),
        local_count: 0,
    };
    assert!(decode_resident_with_prefix(reader(&bytes), body, &root, Some(custody)).is_err());
    assert!(decode_resident(reader(&bytes), body, None).is_err());
}

#[cfg(all(
    test,
    any(feature = "observation-store", feature = "local-evidence-store")
))]
#[path = "native_restart_rewrite_tests.rs"]
mod prepared_rewrite;

#[path = "native_restart_network_tests.rs"]
mod network_tests;
