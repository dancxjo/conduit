use super::*;
use conduit_body::{Body, BodyBiographyEvidence, BodyMembership};
use conduit_core::{CheckedFormId, SourceDocumentId};

#[test]
fn bounded_validated_biography_evidence_round_trips_separately_from_admission() {
    let body = Body::born(
        SourceDocumentId::from("source/browser-biography-frame"),
        CheckedFormId::from("checked/browser-biography-frame"),
        1,
        SignId::from("sign/browser-biography-frame/born"),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Biography frame".into()).unwrap();
    let frame = BrowserAdmissionEgress::BiographyEvidence {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        evidence: Box::new(evidence.clone()),
    };
    let mut output = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
    let length = encode_browser_admission_frame(&frame, &mut output).unwrap();
    assert_eq!(
        serde_json::from_slice::<BrowserAdmissionEgress>(&output[..length]).unwrap(),
        frame
    );

    let mut malformed = evidence;
    malformed.body_id = serde_json::from_str("\"body/wrong\"").unwrap();
    assert_eq!(
        encode_browser_admission_frame(
            &BrowserAdmissionEgress::BiographyEvidence {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                evidence: Box::new(malformed),
            },
            &mut output,
        ),
        Err(BrowserAdmissionFrameError::InvalidBiographyEvidence)
    );
}
