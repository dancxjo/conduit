use super::*;
use conduit_body::{AdmissionManager, Body, BodyMembership, SpawnInvitationSecret};
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};

fn body(seed: &str) -> BodyBiographyEvidence {
    let body = Body::born(
        SourceDocumentId::from("source/authoring"),
        CheckedPlotId::from("checked/authoring"),
        1,
        SignId::from(seed),
    )
    .unwrap();
    BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Hamm Creek House".into(),
    )
    .unwrap()
}
fn claim(body: &BodyBiographyEvidence) -> SpawnInvitationClaim {
    AdmissionManager::new(body.body_id.clone())
        .unwrap()
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
            [8; 32],
            10,
            1000,
        )
        .unwrap()
        .claim()
}
fn checked(name: &str) -> CheckedHostConfiguration {
    let source = include_str!("../../../../targets/std/profiles/linux-computer.host.conduit")
        .replace("host linux-computer", &format!("host {name}"));
    conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}
fn prepare(
    body: &BodyBiographyEvidence,
    claim: &SpawnInvitationClaim,
    now: u64,
) -> Result<CheckedBodyDescription, String> {
    check_joining_host(
        body,
        claim,
        JoiningHostSelection {
            configuration: &checked("second-host"),
            output: SporeOutputKind::NativeBundle,
            deployment: None,
        },
        now,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
}

#[test]
fn joining_host_preserves_birth_and_exact_checked_configuration() {
    let body = body("sign/original-birth");
    let before = serde_json::to_vec(&body).unwrap();
    let invitation = claim(&body);
    for name in ["second-host", "another-host"] {
        let configuration = checked(name);
        let result = check_joining_host(
            &body,
            &invitation,
            JoiningHostSelection {
                configuration: &configuration,
                output: SporeOutputKind::NativeBundle,
                deployment: None,
            },
            20,
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap();
        assert_eq!(result.description().body.id, body.body_id.as_str());
        assert_eq!(
            result.hosts()[0].configuration.configuration_id(),
            configuration.configuration_id()
        );
        assert_eq!(
            result.hosts()[0].description.spore.invitation.as_deref(),
            Some(invitation.invitation_id.as_str())
        );
        assert_eq!(
            result.hosts()[0].description.spore.join_mode,
            SporeJoinMode::SelfJoining
        );
    }
    assert_eq!(serde_json::to_vec(&body).unwrap(), before);
}

#[test]
fn joining_host_refuses_wrong_body_expiry_and_corrupt_biography() {
    let body = body("sign/original-birth");
    let invitation = claim(&body);
    assert!(prepare(&body, &invitation, 1000)
        .unwrap_err()
        .contains("Expired"));
    let other = self::body("sign/other-birth");
    assert!(prepare(&body, &claim(&other), 20)
        .unwrap_err()
        .contains("different Body"));
    let mut corrupt = body.clone();
    corrupt.body_id = other.body_id;
    assert!(prepare(&corrupt, &invitation, 20)
        .unwrap_err()
        .contains("Body evidence"));
}
