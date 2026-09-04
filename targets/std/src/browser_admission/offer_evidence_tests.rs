use super::*;
use conduit_body::{
    CapabilitySummary, HostOfferProjection, OfferDisclosureStage, RemoteProofClass,
};

#[test]
fn admitted_offer_evidence_round_trips_without_becoming_planning_detail() {
    let evidence = HostOfferProjection {
        stage: OfferDisclosureStage::AdmittedMembership,
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser/offer-frame"),
        boot_id: BootId::from("browser-boot/offer-frame"),
        offer_generation: OfferGeneration(1),
        observation_sign_id: SignId::from("sign/browser/offer-frame"),
        freshness_sequence: 1,
        proof_class: RemoteProofClass::SelfReported,
        profile: Some(HostProfileId::from("browser/profile")),
        capability_summary: vec![CapabilitySummary {
            capability_id: conduit_core::CapabilityId::from("browser/capability"),
            implementation_id: conduit_core::ImplementationId::from("browser/implementation"),
        }],
        capabilities: vec![],
        resources: vec![],
    };
    let frame = BrowserAdmissionEgress::OfferEvidence {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        evidence: Box::new(evidence.clone()),
    };
    let mut output = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
    let length = encode_browser_admission_frame(&frame, &mut output).unwrap();
    assert_eq!(
        serde_json::from_slice::<BrowserAdmissionEgress>(&output[..length]).unwrap(),
        frame
    );

    let mut planning_detail = evidence;
    planning_detail.stage = OfferDisclosureStage::Planning;
    assert_eq!(
        encode_browser_admission_frame(
            &BrowserAdmissionEgress::OfferEvidence {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                evidence: Box::new(planning_detail),
            },
            &mut output,
        ),
        Err(BrowserAdmissionFrameError::InvalidOfferEvidence)
    );
}
