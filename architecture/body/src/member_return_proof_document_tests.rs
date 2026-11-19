use super::*;
use alloc::vec;

fn proof() -> PartReturnProof {
    PartReturnProof {
        admission_id: AdmissionId::bound("admission/return".into()),
        body_id: BodyId::bound("body/house".into()),
        part_id: PartId::bound("part/moon".into()),
        host_id: "host/moon".into(),
        boot_id: "boot/current".into(),
        nonce: [3; 32],
        signature: [7; 64],
    }
}
#[test]
fn canonical_wire_round_trip_preserves_exact_proof_without_authenticating_it() {
    let original = proof();
    let bytes = serde_json::to_vec(&MemberReturnProofDocument::encode(original.clone())).unwrap();
    let document: MemberReturnProofDocument = serde_json::from_slice(&bytes).unwrap();
    assert!(document.decode().unwrap() == original);
}
#[test]
fn malformed_signature_extents_refuse_before_owner_proof_verification() {
    for extent in [0, 63, 65] {
        let mut document = MemberReturnProofDocument::encode(proof());
        document.signature = vec![7; extent];
        assert!(matches!(
            document.decode(),
            Err(MemberReturnProofDocumentRefusal::SignatureExtent)
        ));
    }
}
#[test]
fn each_lifecycle_identifier_has_a_finite_nonempty_extent() {
    for field in ["admission_id", "body_id", "part_id", "host_id", "boot_id"] {
        for extent in [0, crate::MAX_LIFECYCLE_ID_BYTES + 1] {
            let mut value =
                serde_json::to_value(MemberReturnProofDocument::encode(proof())).unwrap();
            value[field] = "x".repeat(extent).into();
            let document: MemberReturnProofDocument = serde_json::from_value(value).unwrap();
            assert!(matches!(
                document.decode(),
                Err(MemberReturnProofDocumentRefusal::IdentifierBounds)
            ));
        }
    }
}
