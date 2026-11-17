//! Real owner admission and complete-workset planning, without opening a carrier.
use super::*;
use conduit_body::{
    MembershipCredential, PortableSpawnAdmissionRequest, RendezvousAuthentication,
    RendezvousCandidate, RendezvousLineFamily, SpawnInvitationSecret,
    SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::OfferGeneration;
use conduit_std_host::{StdHost, StdHostConfig};

fn selected(owner: &mut Owner, root: &Path) -> StartupMemberSelection {
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/startup-member".into(),
        boot_id: "boot/startup-member".into(),
        offer_generation: OfferGeneration(1),
    });
    let advertisement = host.advertisement().clone();
    let key = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: advertisement.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &advertisement.host_id,
                &advertisement.boot_id,
                advertisement.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let credential: MembershipCredential = owner
        .admit_invited(root, request, advertisement.host_id.as_str())
        .unwrap()
        .credential;
    StartupMemberSelection {
        member: MemberPresentationBasis::new(credential, advertisement).unwrap(),
        rendezvous: RunningHostRendezvousDescriptor::new(
            vec![RendezvousCandidate {
                candidate_id: "candidate/startup-test".into(),
                line_family: RendezvousLineFamily::AuthenticatedTlsStream,
                reachability: "wss://member.example:443/startup".into(),
                authentication: RendezvousAuthentication {
                    server_identity: "member.example".into(),
                    transport_binding_sha256: [17; 32],
                },
                expires_at_millis: 10_000,
                maximum_attempts: 1,
                attempt_timeout_millis: 1000,
            }],
            [19; 32],
            1000,
        )
        .unwrap(),
        candidate_id: "candidate/startup-test".into(),
        authority_grants: vec![],
        protected_resource_grants: vec![],
    }
}

#[test]
fn member_planning_covers_both_retained_plots_and_keeps_the_original_body() {
    let (root, mut owner) = super::super::service_workset_tests::fixture();
    let selection = selected(&mut owner, &root);
    let body = owner.session.evidence().body_id.clone();
    let workset = owner.session.evidence().body.workset.clone();
    let sources = super::super::super::retained_source::workset(&root)
        .unwrap()
        .unwrap();
    let borrowed: Vec<_> = sources.iter().map(|s| (&s.source, &s.plot)).collect();
    owner
        .plan_startup_member_workset(
            &borrowed,
            &selection,
            &selection.member.host_advertisement,
            1000,
        )
        .unwrap();
    let proposed = owner.session.realization().unwrap();
    assert_eq!(proposed.plan.body_id, body);
    assert_eq!(proposed.plan.workset, workset);
    assert_eq!(proposed.plan.plots.len(), 2);
    assert!(proposed.play.is_none());
    assert!(proposed.plan.plots.iter().all(|plot| {
        plot.plan.fragments.len() == 1
            && plot.plan.fragments[0].host_id == selection.member.host_advertisement.host_id
    }));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_retained_plot_or_changed_member_truth_cannot_publish_a_proposal() {
    let (root, mut owner) = super::super::service_workset_tests::fixture();
    let selection = selected(&mut owner, &root);
    let before = owner.session.evidence().clone();
    let sources = super::super::super::retained_source::workset(&root)
        .unwrap()
        .unwrap();
    let borrowed: Vec<_> = sources.iter().map(|s| (&s.source, &s.plot)).collect();
    assert!(owner
        .plan_startup_member_workset(
            &borrowed[..1],
            &selection,
            &selection.member.host_advertisement,
            1000,
        )
        .is_err());
    let mut changed = selection.member.host_advertisement.clone();
    changed.offer_generation = OfferGeneration(2);
    assert!(owner
        .plan_startup_member_workset(&borrowed, &selection, &changed, 1000)
        .is_err());
    assert!(owner.session.realization().is_none());
    assert_eq!(owner.session.evidence(), &before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn expired_candidate_and_foreign_authority_refuse_before_planning() {
    let (root, mut owner) = super::super::service_workset_tests::fixture();
    let mut selection = selected(&mut owner, &root);
    let actual = selection.member.host_advertisement.clone();
    assert!(selection.validate(&actual, 10_001).is_err());
    selection.authority_grants.push(AuthorityGrant {
        grant_id: "grant/foreign".into(),
        contract_id: "authority/audio".into(),
        host_call_contract_id: "audio/output".into(),
        subject_kind: "sound".into(),
        host_id: "host/foreign".into(),
        boot_id: actual.boot_id.clone(),
        capability_id: "startup-chime".into(),
    });
    assert!(selection.validate(&actual, 1000).is_err());
    assert!(owner.session.realization().is_none());
    std::fs::remove_dir_all(root).unwrap();
}
