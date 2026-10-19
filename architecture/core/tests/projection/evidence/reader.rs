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
