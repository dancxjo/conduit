use super::*;
fn claim() -> SpawnInvitationClaim {
    SpawnInvitationClaim {
        invitation_id: serde_json::from_str("\"invitation/test\"").unwrap(),
        body_id: serde_json::from_str("\"body/test\"").unwrap(),
        nonce: [7; 32],
        expires_at_millis: 1000,
    }
}
#[test]
fn invited_ingress_and_public_challenge_preserve_exact_claim_and_offer() {
    let super::BrowserAdmissionIngress::Advertise { advertisement, .. } =
        super::tests::advertisement()
    else {
        unreachable!()
    };
    let frame = BrowserAdmissionIngress::InvitedAdvertise {
        protocol: 1,
        advertisement,
        claim: claim(),
        verifying_key: vec![8; 32],
    };
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert_eq!(decode_browser_admission_frame(&encoded).unwrap(), frame);
    let mut output = vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
    let length = encode_browser_admission_frame(
        &BrowserAdmissionEgress::SpawnChallenge {
            protocol: 1,
            claim: claim(),
        },
        &mut output,
    )
    .unwrap();
    let decoded: BrowserAdmissionEgress = serde_json::from_slice(&output[..length]).unwrap();
    assert!(
        matches!(decoded, BrowserAdmissionEgress::SpawnChallenge { claim: c, .. } if c == claim())
    );
    let mut invalid = serde_json::from_slice::<serde_json::Value>(&encoded).unwrap();
    invalid["claim"]["nonce"][0] = 256.into();
    assert!(decode_browser_admission_frame(&serde_json::to_vec(&invalid).unwrap()).is_err());
}
#[test]
fn weak_claim_invalid_key_and_wrong_protocol_refuse_without_side_effects() {
    let BrowserAdmissionIngress::Advertise { advertisement, .. } = super::tests::advertisement()
    else {
        unreachable!()
    };
    for (nonce, key, protocol) in [
        ([0; 32], vec![1; 32], 1),
        ([1; 32], vec![1; 31], 1),
        ([1; 32], vec![1; 32], 2),
    ] {
        let mut claim = claim();
        claim.nonce = nonce;
        let frame = BrowserAdmissionIngress::InvitedAdvertise {
            protocol,
            advertisement: advertisement.clone(),
            claim,
            verifying_key: key,
        };
        assert!(decode_browser_admission_frame(&serde_json::to_vec(&frame).unwrap()).is_err());
    }
}
