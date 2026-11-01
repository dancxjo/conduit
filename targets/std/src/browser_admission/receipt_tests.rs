use super::*;

#[test]
fn optional_compact_receipt_is_bounded_and_correlated_without_claiming_root_authentication() {
    use conduit_body::{PortableMembershipAdmissionReceipt, MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA};
    let credential: MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id":"credential/test", "body_id":"body/test", "part_id":"part/test",
        "host_id":"host/browser", "boot_id":"boot/browser", "issued_at_millis":1}))
    .unwrap();
    let receipt = PortableMembershipAdmissionReceipt {
        schema: MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA.into(),
        protocol_version: PROTOCOL_VERSION,
        credential: credential.clone(),
        membership_revision: 1,
        request_digest: [3; 32],
        producer_host: "host/root".into(),
        producer_boot: "boot/root".into(),
        producer_key: [5; 32],
        invitation_id: serde_json::from_str("\"invitation/test\"").unwrap(),
        offer_generation: OfferGeneration(1),
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
        signature: [7; 64],
    };
    let custody = BrowserAdmissionEgress::MembershipReceipt {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        receipt: Box::new(receipt.clone()),
    };
    let mut custody_output = vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
    let count = encode_browser_admission_frame(&custody, &mut custody_output).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&custody_output[..count]).unwrap();
    assert_eq!(document["kind"], "membership-receipt");
    assert!(document.get("credential").is_none());
    assert!(document.get("line_authorization").is_none());
    let mut frame = BrowserAdmissionEgress::Admitted {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        credential: credential.clone(),
        signed_membership_receipt: None,
    };
    let mut output = vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
    let count = encode_browser_admission_frame(&frame, &mut output).unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&output[..count])
            .unwrap()
            .get("signed_membership_receipt")
            .is_none()
    );
    if let BrowserAdmissionEgress::Admitted {
        signed_membership_receipt,
        ..
    } = &mut frame
    {
        *signed_membership_receipt = Some(Box::new(receipt));
    }
    // This is deliberately only structure/correlation. It does not verify the
    // arbitrary fixture signature; Wasm must verify an independent Root pin.
    encode_browser_admission_frame(&frame, &mut output).unwrap();
    if let BrowserAdmissionEgress::Admitted {
        signed_membership_receipt: Some(receipt),
        ..
    } = &mut frame
    {
        receipt.credential.boot_id = "boot/changed".into();
    }
    assert_eq!(
        encode_browser_admission_frame(&frame, &mut output),
        Err(BrowserAdmissionFrameError::InvalidMembershipReceipt)
    );
    if let BrowserAdmissionEgress::Admitted {
        signed_membership_receipt: Some(receipt),
        ..
    } = &mut frame
    {
        receipt.credential = credential;
        receipt.play_created = true;
    }
    assert_eq!(
        encode_browser_admission_frame(&frame, &mut output),
        Err(BrowserAdmissionFrameError::InvalidMembershipReceipt)
    );
}
