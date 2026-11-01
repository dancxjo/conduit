//! Wire inspection preserves claims, including opaque payloads, without admission.
use super::*;
pub(super) fn complete_inventory(bytes: &[u8]) {
    let (_, allocations) = allocation_probe::observe(|| {
        let view = inspect_projection_evidence(bytes).unwrap();
        assert_eq!(view.codec, "test/bits-evidence@1");
        assert_eq!(view.boundary, Some(("expanded/1", "port/result")));
        assert_eq!(view.source_bytes, 7_u64.to_le_bytes());
        assert_eq!(view.target_bytes, Some(3_u64.to_le_bytes().as_slice()));
        assert_eq!(view.policy_bytes, 1_u64.to_le_bytes());
        assert_eq!(view.facts().len(), 3);
        assert!(
            matches!(view.facts().nth(2),Some(ProjectionEvidenceFact::Lost {
            obligation:"2",class:ProjectionLoss::Unrecognized,detail,native_fact:Some("foreign")
        }) if detail==2_u64.to_le_bytes())
        );
        let n = view.native().next().unwrap();
        assert_eq!(
            (n.identity, n.contract, n.provider, n.encoding),
            (
                "foreign",
                "unknown-cost-contract@1",
                "provider@1",
                "opaque-foreign@1"
            )
        );
        assert_eq!(n.bytes, [0xff, 0, 0x80, 7]);
        assert_eq!(view.scores().len(), 2);
        assert!(
            matches!(view.scores().nth(1),Some(ProjectionEvidenceScore::Normalized{fact:"foreign",original,projected,policy:"invert@1"})
            if original.value==17 && projected.value==83 && original.calibration==Some("calibration/native@1") && projected.calibration==Some("calibration/projected@1"))
        );
        assert_eq!(view.admitted().len(), 2);
        let a = view.attempts().nth(1).unwrap();
        assert_eq!(a.realization.back, "back/2");
        assert_eq!(a.fallback_reason, Some("resource-alternative@1"));
        assert_eq!(view.selected_attempt, Some(1));
        assert_eq!(view.mechanism, ProjectionMechanism::Completed);
        assert_eq!(view.diagnostics().count(), 2);
        assert_eq!(view.claimed_summary.preserved, 1);
        assert_eq!(view.claimed_summary.transformed, 1);
        assert_eq!(
            view.claimed_summary.losses[ProjectionLoss::Unrecognized as usize],
            1
        );
    });
    assert_eq!(allocations, 0);
}
fn golden() -> Vec<u8> {
    let hex = include_str!("../../fixtures/projection_evidence_exact_v1.hex").trim();
    let (chunks, remainder) = hex.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    chunks
        .iter()
        .map(|p| u8::from_str_radix(core::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn independent_golden_framing_all_truncations_trailing_and_hostile_lengths() {
    let good = golden();
    let v = inspect_projection_evidence(&good).unwrap();
    assert_eq!(v.facts().count(), 1);
    for n in 0..good.len() {
        assert!(
            inspect_projection_evidence(&good[..n]).is_err(),
            "prefix {n}"
        );
    }
    let mut bad = good.clone();
    bad.push(0);
    assert!(inspect_projection_evidence(&bad).is_err());
    let mut bad = good.clone();
    bad[3] = 2;
    assert_eq!(
        inspect_projection_evidence(&bad).unwrap_err(),
        ProjectionEvidenceReadRefusal::UnsupportedVersion
    );
    for n in [0_u16, 193, u16::MAX] {
        let mut bad = good.clone();
        bad[4..6].copy_from_slice(&n.to_le_bytes());
        assert!(inspect_projection_evidence(&bad).is_err());
    }
    let mut bad = good.clone();
    bad[6] = 0xff;
    assert!(inspect_projection_evidence(&bad).is_err());
    // Claimed summary inventory must match the framing inventory.
    let mut bad = good.clone();
    let n = bad.len();
    bad[n - 14] = 2;
    assert!(inspect_projection_evidence(&bad).is_err());
    let source_start = v.source_bytes.as_ptr() as usize - good.as_ptr() as usize;
    let mut bad = good.clone();
    bad[source_start - 4..source_start].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        inspect_projection_evidence(&bad).unwrap_err(),
        ProjectionEvidenceReadRefusal::Extent
    );
    let target_end = v.target_bytes.unwrap().as_ptr() as usize - good.as_ptr() as usize
        + v.target_bytes.unwrap().len();
    let mut bad = good.clone();
    bad[target_end] = 65;
    assert_eq!(
        inspect_projection_evidence(&bad).unwrap_err(),
        ProjectionEvidenceReadRefusal::Extent
    );
    // Framing cannot establish the domain truth: changing opaque source bytes
    // remains inspectable. Independent admitted-report comparison rejects it.
    let start = v.source_bytes.as_ptr() as usize - good.as_ptr() as usize;
    let mut changed = good.clone();
    changed[start] ^= 1;
    assert!(inspect_projection_evidence(&changed).is_ok());
    let domain = Bits;
    let policy = Policy(0);
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let report =
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &facts)).unwrap();
    assert!(verify_projection_evidence(&report, &Codec(0), limits(), &changed).is_err());
}

#[test]
fn every_loss_class_round_trips_with_opaque_native_custody_without_allocation() {
    let source = 1;
    let target = 0;
    let detail = 0;
    let native = [ProjectionNativeFact {
        identity: text("original"),
        contract: text("unknown-native@1"),
        provider: text("external@1"),
        encoding: text("opaque@1"),
        bytes: &[0xff, 0x80, 0],
    }];
    for class in [
        ProjectionLoss::Unrepresentable,
        ProjectionLoss::Unrecognized,
        ProjectionLoss::Precision,
        ProjectionLoss::Range,
        ProjectionLoss::Cardinality,
        ProjectionLoss::Ordering,
        ProjectionLoss::TemporalFinality,
        ProjectionLoss::Identity,
        ProjectionLoss::Provenance,
        ProjectionLoss::UnsupportedVariant,
        ProjectionLoss::Truncation,
        ProjectionLoss::Approximation,
    ] {
        let facts = [ProjectionFact::Lost {
            obligation: text("0"),
            class,
            detail: &detail,
            native_fact: Some(text("original")),
        }];
        let mut draft = input(&source, Some(&target), &facts);
        draft.native = &native;
        let report = ProjectionReport::new(&Bits, &Policy(1), draft).unwrap();
        let mut bytes = [0; 4096];
        let (_, allocations) = allocation_probe::observe(|| {
            let n = encode_projection_evidence(&report, &Codec(1), limits(), &mut bytes).unwrap();
            verify_projection_evidence(&report, &Codec(1), limits(), &bytes[..n]).unwrap();
            let view = inspect_projection_evidence(&bytes[..n]).unwrap();
            assert_eq!(view.claimed_summary, report.summary());
            assert_eq!(view.claimed_summary.losses[class as usize], 1);
            for _ in 0..3 {
                let mut facts = view.facts();
                assert_eq!(facts.len(), 1);
                assert!(matches!(facts.next(), Some(ProjectionEvidenceFact::Lost {
                    class: actual, native_fact: Some("original"), detail, ..
                }) if actual == class && detail == 0_u64.to_le_bytes()));
                assert_eq!(facts.len(), 0);
                assert!(facts.next().is_none());
                assert!(facts.next().is_none());
                assert_eq!(view.native().next().unwrap().bytes, native[0].bytes);
            }
        });
        assert_eq!(allocations, 0);
    }
}

#[test]
fn invalid_tags_and_inconsistent_optional_target_refuse_before_domain_replay() {
    let good = golden();
    let view = inspect_projection_evidence(&good).unwrap();
    let source_end =
        view.source_bytes.as_ptr() as usize - good.as_ptr() as usize + view.source_bytes.len();
    let target = view.target_bytes.unwrap();
    let target_end = target.as_ptr() as usize - good.as_ptr() as usize + target.len();
    // Source bytes are followed by the optional target tag. It is a closed
    // framing tag, not an arbitrary domain byte or a target-presence guess.
    for tag in [2, 0xff] {
        let mut bad = good.clone();
        bad[source_end] = tag;
        assert_eq!(
            inspect_projection_evidence(&bad).unwrap_err(),
            ProjectionEvidenceReadRefusal::Malformed
        );
    }
    let mut bad = good.clone();
    // Keep the original target identity, but remove its optional payload.
    bad.drain(source_end + 1..target_end);
    bad[source_end] = 0;
    assert_eq!(
        inspect_projection_evidence(&bad).unwrap_err(),
        ProjectionEvidenceReadRefusal::Malformed
    );
    let mut bad = good.clone();
    // The first fact follows the count after the complete target payload.
    bad[target_end + 1] = 3;
    assert_eq!(
        inspect_projection_evidence(&bad).unwrap_err(),
        ProjectionEvidenceReadRefusal::Malformed
    );
}

struct ChangingCodec(core::cell::Cell<usize>);
impl ProjectionEvidenceCodec<Bits> for ChangingCodec {
    fn identity(&self) -> ProjectionText<'_> {
        text("test/changing-codec@1")
    }
    fn configuration_digest(&self) -> [u8; 32] {
        configuration(0)
    }
    fn source(
        &self,
        value: &u64,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        let count = self.0.get() + 1;
        self.0.set(count);
        if count == 3 {
            // The final writing callback contradicts its prior measurement.
            Ok(())
        } else {
            out.u64(*value)
        }
    }
    fn target(
        &self,
        value: &u64,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        Codec(0).target(value, out)
    }
    fn detail(
        &self,
        value: &usize,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        Codec(0).detail(value, out)
    }
    fn policy(
        &self,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        Codec(0).policy(out)
    }
}

#[test]
fn third_pass_extent_mismatch_refuses_without_hiding_staging_or_callback_work() {
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let report =
        ProjectionReport::new(&Bits, &Policy(0), input(&source, Some(&source), &facts)).unwrap();
    let codec = ChangingCodec(core::cell::Cell::new(0));
    let mut staging = [0xa5; 4096];
    let (result, allocations) = allocation_probe::observe(|| {
        encode_projection_evidence(&report, &codec, limits(), &mut staging)
    });
    assert_eq!(result, Err(ProjectionEvidenceRefusal::DomainEncoding));
    assert_eq!(codec.0.get(), 3);
    assert_eq!(allocations, 0);
    // Failure does not promise staging restoration. Without a successful span
    // the caller must not publish any of these partially written bytes.
    assert_ne!(staging, [0xa5; 4096]);
}
