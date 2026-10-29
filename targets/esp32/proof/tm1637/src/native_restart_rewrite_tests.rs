//! Prepared checkpoint custody contract; reuses the actual parent fixtures.
use super::*;

#[cfg(any(feature = "observation-store", feature = "local-evidence-store"))]
#[test]
fn authenticated_rewrite_freezes_receipts_and_reuses_signature_validation() {
    use crate::native_body_checkpoint::{Checkpoint, Refusal};
    use sha2::{Digest, Sha256};
    let (segments, evidence) = fixture();
    let body = &evidence.body_id;
    let seed = [61; 32];
    let key = SigningKey::from_bytes(&seed);
    let root = key.verifying_key().to_bytes();
    let prefix = receipt(body, &segments, &key);
    let mut layouts = layout::Layouts::default();
    layouts
        .add(&super::super::super::observed::tests::receipt(
            body, &seed, 1,
        ))
        .unwrap();
    let manager = AdmissionManager::new(body.clone()).unwrap();
    let evidence_json: serde_json::Value =
        serde_json::from_slice(&plain_v2(&evidence, &prefix, &[])).unwrap();
    let plain=format!("{{\"schema\":{},\"evidence\":{},\"biography_prefix\":{},\"archives\":[],\"admissions\":{},\"observation_layouts\":{}}}",serde_json::to_string(SCHEMA_V3).unwrap(),evidence_json["evidence"],serde_json::to_string(&prefix).unwrap(),serde_json::to_string(&manager).unwrap(),serde_json::to_string(&layouts).unwrap()).into_bytes();
    let binding: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    let mut journal = Checkpoint::open(crate::native_evidence_region::test_empty()).unwrap();
    journal.save(&binding, &seed, &plain).unwrap();
    let mut checked_seed = [0; 32];
    for mode in [1, 2] {
        crate::checkpoint_decode_fault(mode);
        checked_seed.fill(0);
        let refused = super::super::super::decode_checkpoint(
            &mut journal,
            &binding,
            body,
            &mut checked_seed,
            None,
        );
        crate::checkpoint_decode_fault(0);
        if mode == 1 {
            assert_eq!(refused.err(), Some("restart-key-changed"));
        } else {
            assert!(
                refused.is_err(),
                "changed second source must not mint proof"
            );
        }
        assert_eq!(checked_seed, [0; 32]);
    }
    let before = rewrite::verification_passes();
    let (resident, source) = super::super::super::decode_checkpoint(
        &mut journal,
        &binding,
        body,
        &mut checked_seed,
        None,
    )
    .unwrap();
    assert_eq!(rewrite::verification_passes() - before, 1);
    let prepare = |root: &[u8; 32], prefix: Option<&wire::Receipt>, layouts: &layout::Layouts| {
        // Returned recipe deliberately cannot escape the caller's immutable
        // borrow scope. This closure observes only acceptance/refusal.
        rewrite::PreparedRewrite::prepare(
            resident.proof.as_ref().unwrap(),
            &source,
            &evidence,
            &[],
            &manager,
            &resident.summary,
            resident.span,
            prefix,
            0,
            root,
            layouts,
        )
        .map(|_| ())
    };
    assert!(prepare(&root, Some(&prefix), &layouts).is_ok());
    assert!(prepare(
        &SigningKey::from_bytes(&[62; 32]).verifying_key().to_bytes(),
        Some(&prefix),
        &layouts
    )
    .is_err());
    let mut forged = prefix.clone();
    forged.signature.0[0] ^= 1;
    assert!(prepare(&root, Some(&forged), &layouts).is_err());
    assert!(prepare(&root, None, &layouts).is_err());
    let mut changed_layouts = layout::Layouts::default();
    changed_layouts
        .add(&super::super::super::observed::tests::receipt(
            body, &seed, 2,
        ))
        .unwrap();
    assert!(prepare(&root, Some(&prefix), &changed_layouts).is_err());
    let foreign =
        conduit_body::Body::born("foreign".into(), "foreign".into(), 1, "born".into()).unwrap();
    let foreign_manager = AdmissionManager::new(foreign.body_id).unwrap();
    assert!(rewrite::PreparedRewrite::prepare(
        resident.proof.as_ref().unwrap(),
        &source,
        &evidence,
        &[],
        &foreign_manager,
        &resident.summary,
        resident.span,
        Some(&prefix),
        0,
        &root,
        &layouts
    )
    .is_err());
    assert!(rewrite::PreparedRewrite::prepare(
        resident.proof.as_ref().unwrap(),
        &source,
        &evidence,
        &segments[..1],
        &manager,
        &resident.summary,
        resident.span,
        Some(&prefix),
        0,
        &root,
        &layouts
    )
    .is_err());
    let (prepared, preparation_memory) = crate::allocation_probe::observe(|| {
        rewrite::PreparedRewrite::prepare(
            resident.proof.as_ref().unwrap(),
            &source,
            &evidence,
            &[],
            &manager,
            &resident.summary,
            resident.span,
            Some(&prefix),
            0,
            &root,
            &layouts,
        )
        .unwrap()
    });
    println!(
        "rewrite completion inline={} optional proof inline={}",
        core::mem::size_of::<crate::native_body_checkpoint::RewriteCompletion>(),
        core::mem::size_of::<Option<rewrite::AuthenticatedReceipts>>()
    );
    println!(
        "receipt proof inline={} recipe inline={} prepared allocations={} peak={} retained={}",
        core::mem::size_of::<rewrite::AuthenticatedReceipts>(),
        core::mem::size_of_val(&prepared),
        preparation_memory.allocations,
        preparation_memory.peak_bytes,
        preparation_memory.retained_bytes()
    );
    assert_eq!(
        preparation_memory.retained_bytes(),
        evidence.body_id.as_str().len(),
        "only the computed cumulative Summary owns a new Body ID; proof adds no heap"
    );
    let before = rewrite::verification_passes();
    let mut prepared_bytes = Vec::new();
    for _ in 0..2 {
        let mut emitted = Vec::new();
        journal
            .read_snapshot(&source, |reader| {
                prepared
                    .emit(
                        |out| reader.read(out).map_err(|_| ()),
                        &mut |bytes: &[u8]| {
                            emitted.extend_from_slice(bytes);
                            Ok::<_, Refusal>(())
                        },
                    )
                    .map_err(|_| Refusal::Corrupt)
            })
            .unwrap();
        if prepared_bytes.is_empty() {
            prepared_bytes = emitted;
        } else {
            assert_eq!(prepared_bytes, emitted);
        }
    }
    assert_eq!(
        rewrite::verification_passes(),
        before,
        "both frozen passes reuse checked receipts"
    );
    for _ in 0..2 {
        let mut raw_bytes = Vec::new();
        journal
            .read_snapshot(&source, |reader| {
                stream_rewrite_with_layout(
                    &evidence,
                    &[],
                    &manager,
                    &resident.summary,
                    resident.span,
                    Some(&prefix),
                    0,
                    &root,
                    &layouts,
                    |out| reader.read(out).map_err(|_| ()),
                    &mut |bytes: &[u8]| {
                        raw_bytes.extend_from_slice(bytes);
                        Ok::<_, Refusal>(())
                    },
                )
                .map_err(|_| Refusal::Corrupt)
            })
            .unwrap();
        assert_eq!(raw_bytes, prepared_bytes);
    }
    assert_eq!(
        rewrite::verification_passes() - before,
        2,
        "raw entrance retains verification on each call"
    );
    let result = journal.read_snapshot(&source, |reader| {
        prepared
            .emit(
                |out| reader.read(out).map_err(|_| ()),
                &mut |_bytes: &[u8]| Err::<(), _>(Refusal::Storage),
            )
            .map_err(|_| Refusal::Storage)
    });
    assert!(result.is_err());
    assert_eq!(journal.generation(), Some(source.generation()));
    // A genuine raw writer can commit a numerically valid successor with a
    // different receipt set. That storage fact must not carry this recipe's
    // semantic proof, even though every replacement signature is valid.
    let expected_encoded: [u8; 32] = Sha256::digest(&prepared_bytes).into();
    for changed_seed in [false, true] {
        let mut other = Checkpoint::open(crate::native_evidence_region::test_empty()).unwrap();
        other.save(&binding, &seed, &plain).unwrap();
        let (_other, other_source) = super::super::super::decode_checkpoint(
            &mut other,
            &binding,
            body,
            &mut checked_seed,
            None,
        )
        .unwrap();
        assert!(other_source.identity() == source.identity());
        let next_seed = if changed_seed { [62; 32] } else { seed };
        let substituted = other
            .rewrite_streamed_attested(&other_source, &binding, &next_seed, |io| {
                if changed_seed {
                    prepared
                        .emit(|out| io.read_source(out).map_err(|_| ()), &mut |bytes| {
                            io.write(bytes)
                        })
                        .map(|_| ())
                        .map_err(|_| Refusal::Corrupt)
                } else {
                    stream_rewrite_with_layout(
                        &evidence,
                        &[],
                        &manager,
                        &resident.summary,
                        resident.span,
                        Some(&prefix),
                        0,
                        &root,
                        &changed_layouts,
                        |out| io.read_source(out).map_err(|_| ()),
                        &mut |bytes| io.write(bytes),
                    )
                    .map(|_| ())
                    .map_err(|_| Refusal::Corrupt)
                }
            })
            .unwrap();
        assert!(substituted.source().succeeds(source.identity()));
        assert_eq!(
            substituted.encoded_matches(prepared_bytes.len(), &expected_encoded),
            changed_seed
        );
        assert_eq!(substituted.retains_seed(), !changed_seed);
        assert!(
            prepared.committed(substituted).is_err(),
            "changed receipts OR changed header signer cannot inherit old receipt custody"
        );
    }
    // Even identical encoded bytes plus unchanged signer relative to a DIFFERENT
    // old authenticated source cannot inherit this recipe's source custody.
    let mut foreign_source = Checkpoint::open(crate::native_evidence_region::test_empty()).unwrap();
    let other_seed = [62; 32];
    foreign_source.save(&binding, &other_seed, &plain).unwrap();
    let (_, foreign_snapshot) = foreign_source
        .load_streamed_with_snapshot(&binding, &mut checked_seed, |reader| {
            let mut chunk = [0; 128];
            while reader.read(&mut chunk)? != 0 {}
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert!(foreign_snapshot.identity() != source.identity());
    let different = foreign_source
        .rewrite_streamed_attested(&foreign_snapshot, &binding, &other_seed, |io| {
            prepared
                .emit(|out| io.read_source(out).map_err(|_| ()), &mut |bytes| {
                    io.write(bytes)
                })
                .map(|_| ())
                .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    assert!(different.encoded_matches(prepared_bytes.len(), &expected_encoded));
    assert!(different.retains_seed());
    assert!(!different.continues_source(source.identity()));
    assert!(prepared.committed(different).is_err());
    let mut calls = 0;
    let completion = journal
        .rewrite_streamed_attested(&source, &binding, &seed, |io| {
            calls += 1;
            prepared
                .emit(|out| io.read_source(out).map_err(|_| ()), &mut |bytes| {
                    io.write(bytes)
                })
                .map(|_| ())
                .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    assert_eq!(calls, 2);
    let (successor, carried) = prepared.committed(completion).unwrap();
    let (next, next_source) = super::super::super::decode_checkpoint(
        &mut journal,
        &binding,
        body,
        &mut checked_seed,
        None,
    )
    .unwrap();
    assert!(next_source.identity() == successor.identity());
    assert!(
        rewrite::PreparedRewrite::prepare(
            resident.proof.as_ref().unwrap(),
            &next_source,
            &evidence,
            &[],
            &manager,
            &next.summary,
            next.span,
            Some(&prefix),
            0,
            &root,
            &layouts
        )
        .is_err(),
        "old source proof cannot follow changed generation/bank/digest"
    );
    rewrite::PreparedRewrite::prepare(
        &carried,
        &next_source,
        &evidence,
        &[],
        &manager,
        &next.summary,
        next.span,
        Some(&prefix),
        0,
        &root,
        &layouts,
    )
    .unwrap();
    for malformed in [
        plain[..plain.len() - 1].to_vec(),
        [plain.as_slice(), b" trailing"].concat(),
    ] {
        let mut invalid = Checkpoint::open(crate::native_evidence_region::test_empty()).unwrap();
        invalid.save(&binding, &seed, &malformed).unwrap();
        assert!(
            super::super::super::decode_checkpoint(
                &mut invalid,
                &binding,
                body,
                &mut checked_seed,
                None
            )
            .is_err(),
            "authenticated malformed JSON cannot mint receipt proof"
        );
        assert_eq!(checked_seed, [0; 32]);
    }
}
