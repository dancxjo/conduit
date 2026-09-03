use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationChoice,
    BodyGraduationEvidence, BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{bind_sign, CheckedFormId, OfferGeneration, SignId, SourceDocumentId};

const HOST: &str = "browser/patchbay-test";
const BOOT: &str = "browser-boot/patchbay-test";
const PLAN: &str = "plan/patchbay-test";
const IMPLEMENTATION: &str = "browser/patchbay-surface@1";

fn evidence() -> Vec<u8> {
    let host = HostId::from(HOST);
    let boot = BootId::from(BOOT);
    let born_sign = bind_sign(&host, &boot, None, 1).sign_id;
    let body = Body::born(
        SourceDocumentId::from("source/patchbay-test"),
        CheckedFormId::from("checked/patchbay-test"),
        1,
        born_sign,
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part = PartId::bind(&body.body_id, "patchbay/browser", 1).unwrap();
    let proof = MembershipProofId::bind("proof/patchbay-browser").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from("sign/patchbay-admitted"),
        )
        .unwrap();
    let joined = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host,
                boot_id: boot,
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            SignId::from("sign/patchbay-joined"),
        )
        .unwrap();
    let mut biography = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone()).unwrap(),
        "Test Body".into(),
        "test-form@1".into(),
    )
    .unwrap();
    biography
        .append_membership_events(membership, &[(admitted, 2), (joined, 3)])
        .unwrap();
    biography
        .graduate(BodyGraduationEvidence {
            body_id: biography.body_id.clone(),
            sequence: 4,
            sign_id: SignId::from("sign/patchbay-graduated"),
            choice: BodyGraduationChoice::HostedPatchbay,
            patchbay_plan_id: Some(PlanId::from(PLAN)),
            patchbay_implementation_id: Some(ImplementationId::from(IMPLEMENTATION)),
        })
        .unwrap();
    serde_json::to_vec(&biography).unwrap()
}

fn invoke(
    mode: u32,
    host: &str,
    boot: &str,
    plan: &str,
    implementation: &str,
    evidence: &[u8],
) -> i32 {
    let mut bytes = Vec::new();
    for value in [
        host.as_bytes(),
        boot.as_bytes(),
        plan.as_bytes(),
        implementation.as_bytes(),
        evidence,
    ] {
        bytes.extend_from_slice(value);
    }
    INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    conduit_patchbay_open_body(
        mode,
        host.len(),
        boot.len(),
        plan.len(),
        implementation.len(),
        evidence.len(),
    )
}

#[test]
fn hosted_and_external_open_the_same_evidence_without_conflating_membership() {
    let evidence = evidence();
    assert_eq!(invoke(1, "", "", "", "", &evidence), STATUS_READY);
    let external: serde_json::Value =
        OUTPUT.with(|output| serde_json::from_slice(&output.borrow()).unwrap());
    assert_eq!(external["relationship"], "external");
    assert!(external["current_host_id"].is_null());

    assert_eq!(
        invoke(2, HOST, BOOT, PLAN, IMPLEMENTATION, &evidence),
        STATUS_READY
    );
    let hosted: serde_json::Value =
        OUTPUT.with(|output| serde_json::from_slice(&output.borrow()).unwrap());
    assert_eq!(hosted["relationship"], "hosted");
    assert_eq!(hosted["current_boot_id"], BOOT);
    assert_eq!(hosted["body_id"], external["body_id"]);
}

#[test]
fn hosted_open_requires_exact_current_boot_and_placement() {
    let evidence = evidence();
    assert_eq!(
        invoke(
            2,
            HOST,
            "browser-boot/stale",
            PLAN,
            IMPLEMENTATION,
            &evidence
        ),
        ERROR_HOSTED_MEMBERSHIP
    );
    assert_eq!(
        invoke(2, HOST, BOOT, "plan/wrong", IMPLEMENTATION, &evidence),
        ERROR_EVIDENCE
    );
    assert_eq!(invoke(1, "", "", "", "", b"{}"), ERROR_EVIDENCE);
}
