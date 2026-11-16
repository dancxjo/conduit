//! Deterministic make proof, without a browser launch or admission claim.
use super::*;
use conduit_body::{AdmissionManager, Body, BodyMembership};

fn origin() -> BodyBiographyEvidence {
    let body = Body::born(
        "source/propagation".into(),
        "checked/propagation".into(),
        1,
        SignId::from("sign/propagation-born"),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    BodyBiographyEvidence::born(body, membership, "Propagation".into()).unwrap()
}

fn invitation(body: &BodyBiographyEvidence) -> SpawnInvitationClaim {
    AdmissionManager::new(body.body_id.clone())
        .unwrap()
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            1_000,
            2_000,
        )
        .unwrap()
        .claim()
}

fn selection() -> BrowserConfigurationSelection {
    BrowserConfigurationSelection {
        catalog_generation: 1,
        implementations: vec![
            "browser/dom@1".into(),
            "browser/keyboard-events@1".into(),
            "browser/pointer-events@1".into(),
        ],
    }
}

#[test]
fn preparing_browser_spore_preserves_origin_and_does_not_admit_a_part() {
    let body = origin();
    let before = body.clone();
    let claim = invitation(&body);
    let prepared = prepare_workspace_browser(
        &body,
        &claim,
        &[11; 32],
        &format!("sha256:{}", "1".repeat(64)),
        selection(),
        1_001,
    )
    .unwrap();
    assert_eq!(body, before);
    assert_eq!(prepared.body_id, body.body_id.as_str());
    assert_eq!(prepared.spore_manifest.body_id, body.body_id.as_str());
    assert_eq!(prepared.source_identity, "sign/propagation-born");
    assert_eq!(
        prepared.spore_manifest.host_configuration_id,
        prepared.browser_configuration_id.as_deref().unwrap()
    );
    assert_eq!(
        prepared.image_content_digest,
        format!("sha256:{}", "1".repeat(64))
    );
    assert_eq!(
        prepared.spore_manifest.binding,
        SporeBinding::SelfJoining {
            invitation_id: claim.invitation_id.as_str().into(),
        }
    );
    assert_eq!(prepared.disposition, "prepared");
    assert_eq!(
        prepared.does_not_prove,
        [
            "deployment",
            "boot",
            "join",
            "membership",
            "offers",
            "readiness"
        ]
    );
}

#[test]
fn another_body_invitation_refuses_before_configuration_or_make() {
    let body = origin();
    let mut claim = invitation(&body);
    claim.body_id = Body::born(
        "source/propagation".into(),
        "checked/propagation".into(),
        1,
        SignId::from("sign/other-born"),
    )
    .unwrap()
    .body_id;
    let mut invalid_selection = selection();
    invalid_selection.catalog_generation = 0;
    let refused = prepare_workspace_browser(
        &body,
        &claim,
        &[11; 32],
        "invalid-digest",
        invalid_selection,
        1_001,
    )
    .err()
    .unwrap();
    assert_eq!(refused, "Workspace invitation names a different Body");
}

#[test]
fn expired_or_stale_invitation_cannot_prepare_a_browser_spore() {
    let body = origin();
    let before = body.clone();
    let claim = invitation(&body);
    let digest = format!("sha256:{}", "1".repeat(64));
    let expired = prepare_workspace_browser(&body, &claim, &[11; 32], &digest, selection(), 2_000)
        .err()
        .unwrap();
    assert!(expired.contains("Expired"), "{expired}");
    let mut stale = claim;
    stale.nonce = [0; 32];
    let refused = prepare_workspace_browser(&body, &stale, &[11; 32], &digest, selection(), 1_001)
        .err()
        .unwrap();
    assert!(refused.contains("StaleNonce"), "{refused}");
    assert_eq!(body, before);
}
