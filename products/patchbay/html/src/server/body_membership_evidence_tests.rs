use super::*;
use conduit_body::{AuthenticatedHostObservation, MembershipProofId, PartId};
use conduit_core::{BootId, HostId, OfferGeneration};

fn admission_extension(prior: &BodyBiographyEvidence) -> BodyBiographyEvidence {
    let mut membership = prior.membership.clone();
    let part = PartId::bind(&prior.body_id, "browser/post-birth", 1).unwrap();
    let proof = MembershipProofId::bind("proof/browser/post-birth").unwrap();
    let admitted = membership
        .admit(
            &prior.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from("sign/browser/post-birth/admitted"),
        )
        .unwrap();
    let attached = membership
        .observe_present(
            &prior.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: HostId::from("browser/post-birth"),
                boot_id: BootId::from("browser-boot/post-birth"),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            SignId::from("sign/browser/post-birth/attached"),
        )
        .unwrap();
    let first_sequence = prior.records.last().unwrap().sequence + 1;
    let mut candidate = prior.clone();
    candidate
        .append_membership_events(
            membership,
            &[(admitted, first_sequence), (attached, first_sequence + 1)],
        )
        .unwrap();
    candidate
}

fn leave_extension(prior: &BodyBiographyEvidence) -> BodyBiographyEvidence {
    let mut membership = prior.membership.clone();
    let part = membership.parts.last().unwrap();
    let part_id = part.part_id.clone();
    let boot_id = part.current.as_ref().unwrap().boot_id.clone();
    let detached = membership
        .observe_offline(
            &prior.body_id,
            membership.revision,
            &part_id,
            &boot_id,
            SignId::from("sign/browser/post-birth/left"),
        )
        .unwrap();
    let biography_sequence = prior.records.last().unwrap().sequence + 1;
    let mut candidate = prior.clone();
    candidate
        .append_membership_events(membership, &[(detached, biography_sequence)])
        .unwrap();
    candidate
}

fn last_joined_host(prior: &BodyBiographyEvidence, part_id: &PartId) -> HostId {
    prior
        .records
        .iter()
        .rev()
        .find_map(|record| match &record.kind {
            BodyBiographyRecordKind::HostJoined {
                part_id: joined_part,
                host_id,
                ..
            } if joined_part == part_id => Some(host_id.clone()),
            _ => None,
        })
        .unwrap()
}

fn return_extension(
    prior: &BodyBiographyEvidence,
    host_id: HostId,
    boot_id: BootId,
) -> BodyBiographyEvidence {
    let mut membership = prior.membership.clone();
    let part_id = membership.parts.last().unwrap().part_id.clone();
    let attached = membership
        .observe_present(
            &prior.body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id,
                boot_id,
                offer_generation: OfferGeneration(1),
                proof_id: MembershipProofId::bind("proof/browser/post-birth-return").unwrap(),
                sequence: 2,
            },
            SignId::from("sign/browser/post-birth/returned"),
        )
        .unwrap();
    let biography_sequence = prior.records.last().unwrap().sequence + 1;
    let mut candidate = prior.clone();
    candidate
        .append_membership_events(membership, &[(attached, biography_sequence)])
        .unwrap();
    candidate
}

#[test]
fn exact_admission_leave_and_fresh_boot_return_are_atomic_and_replay_refuses() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let prior = server.body_workload.as_ref().unwrap().evidence().clone();
    let candidate = admission_extension(&prior);
    let encoded = serde_json::to_vec(&candidate).unwrap();

    let adopted = server.apply_body_membership_evidence(&encoded).unwrap();
    let adopted: crate::RendererSnapshot = serde_json::from_slice(&adopted).unwrap();
    let workbench = adopted.body_workbench.unwrap();
    assert_eq!(workbench.evidence_revision, 2);
    assert_eq!(workbench.current["admitted_parts"], 2);
    assert_eq!(
        workbench.current["current_hosts"].as_array().unwrap().len(),
        2
    );
    let admitted = server.body_workload.as_ref().unwrap().evidence().clone();
    let left = leave_extension(&admitted);
    let left_encoded = serde_json::to_vec(&left).unwrap();
    let adopted = server
        .apply_body_membership_evidence(&left_encoded)
        .unwrap();
    let adopted: crate::RendererSnapshot = serde_json::from_slice(&adopted).unwrap();
    let workbench = adopted.body_workbench.unwrap();
    assert_eq!(workbench.evidence_revision, 3);
    assert_eq!(workbench.current["admitted_parts"], 2);
    assert_eq!(
        workbench.current["current_hosts"].as_array().unwrap().len(),
        1
    );
    assert!(server
        .apply_body_membership_evidence(&left_encoded)
        .is_err());
    let part_id = left.membership.parts.last().unwrap().part_id.clone();
    let returned = return_extension(
        &left,
        last_joined_host(&left, &part_id),
        BootId::from("browser-boot/post-birth-return"),
    );
    let returned_encoded = serde_json::to_vec(&returned).unwrap();
    let adopted = server
        .apply_body_membership_evidence(&returned_encoded)
        .unwrap();
    let adopted: crate::RendererSnapshot = serde_json::from_slice(&adopted).unwrap();
    let workbench = adopted.body_workbench.unwrap();
    assert_eq!(workbench.evidence_revision, 4);
    assert_eq!(workbench.current["admitted_parts"], 2);
    assert_eq!(
        workbench.current["current_hosts"].as_array().unwrap().len(),
        2
    );
    assert!(server
        .apply_body_membership_evidence(&returned_encoded)
        .is_err());
}

#[test]
fn return_refuses_stale_boot_and_changed_durable_host() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let prior = PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .unwrap()
        .body_workload
        .as_ref()
        .unwrap()
        .evidence()
        .clone();
    let admitted = admission_extension(&prior);
    let left = leave_extension(&admitted);
    let part_id = left.membership.parts.last().unwrap().part_id.clone();
    let host_id = last_joined_host(&left, &part_id);
    let prior_boot = left
        .records
        .last()
        .and_then(|record| match &record.kind {
            BodyBiographyRecordKind::HostLeft { prior_boot_id, .. } => Some(prior_boot_id.clone()),
            _ => None,
        })
        .unwrap();

    let stale_boot = return_extension(&left, host_id, prior_boot);
    assert!(validate_membership_extension(&left, &stale_boot).is_err());
    let changed_host = return_extension(
        &left,
        HostId::from("browser/different-durable-host"),
        BootId::from("browser-boot/post-birth-return"),
    );
    assert!(validate_membership_extension(&left, &changed_host).is_err());
}

#[test]
fn altered_prior_biography_refuses_without_mutating_current_evidence() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let prior = server.body_workload.as_ref().unwrap().evidence().clone();
    let mut candidate = admission_extension(&prior);
    candidate.friendly_name = "Different Body label".into();
    let encoded = serde_json::to_vec(&candidate).unwrap();

    assert!(server.apply_body_membership_evidence(&encoded).is_err());
    assert_eq!(server.body_workload.as_ref().unwrap().evidence(), &prior);
}

#[test]
fn stale_observer_membership_updates_preserve_local_wake_and_resequence_only_new_records() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let observer = server.body_workload.as_ref().unwrap().evidence().clone();
    let (body, wake) = observer
        .body
        .wake(1, SignId::from("sign/local-woke"))
        .unwrap();
    let sequence = observer.records.last().unwrap().sequence + 1;
    server
        .body_workload
        .as_mut()
        .unwrap()
        .retain_wake(body.clone(), wake.clone(), sequence)
        .unwrap();
    let retained = server.body_workload.as_ref().unwrap().evidence().clone();
    let admitted = admission_extension(&observer);
    server
        .apply_body_membership_evidence(&serde_json::to_vec(&admitted).unwrap())
        .unwrap();
    let local = server.body_workload.as_ref().unwrap().evidence();
    assert_eq!(local.body, body);
    assert_eq!(local.wakes, vec![wake.clone()]);
    assert_eq!(&local.records[..retained.records.len()], retained.records);
    assert_eq!(
        local.records.last().unwrap().sequence,
        admitted.records.last().unwrap().sequence + 1
    );
    assert_eq!(
        local.records.last().unwrap().sign_id,
        admitted.records.last().unwrap().sign_id
    );

    // The observer has not received local Wake history or sequence reassignment.
    let left = leave_extension(&admitted);
    server
        .apply_body_membership_evidence(&serde_json::to_vec(&left).unwrap())
        .unwrap();
    let part_id = left.membership.parts.last().unwrap().part_id.clone();
    let returned = return_extension(
        &left,
        last_joined_host(&left, &part_id),
        BootId::from("browser-boot/post-birth-return"),
    );
    let bytes = serde_json::to_vec(&returned).unwrap();
    server.apply_body_membership_evidence(&bytes).unwrap();
    let local = server.body_workload.as_ref().unwrap().evidence().clone();
    assert_eq!(local.body, body);
    assert_eq!(local.wakes, vec![wake]);
    assert_eq!(local.membership, returned.membership);
    local.validate().unwrap();
    let snapshot = server.encoded_snapshot.clone();
    assert!(server.apply_body_membership_evidence(&bytes).is_err());
    assert_eq!(server.encoded_snapshot, snapshot);
    assert_eq!(server.body_workload.as_ref().unwrap().evidence(), &local);
}

#[test]
fn membership_only_adoption_refuses_an_unknown_body_or_wake_extension() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let prior = server.body_workload.as_ref().unwrap().evidence().clone();
    let mut altered = prior.clone();
    let (body, wake) = prior
        .body
        .wake(1, SignId::from("sign/unknown-wake"))
        .unwrap();
    altered
        .append_wake(body, wake, prior.records.last().unwrap().sequence + 1)
        .unwrap();
    let candidate = admission_extension(&altered);
    candidate.validate().unwrap();
    let snapshot = server.encoded_snapshot.clone();
    assert!(server
        .apply_body_membership_evidence(&serde_json::to_vec(&candidate).unwrap())
        .is_err());
    assert_eq!(server.encoded_snapshot, snapshot);
    assert_eq!(server.body_workload.as_ref().unwrap().evidence(), &prior);
}

#[test]
fn reconciliation_compares_exact_wake_events_not_only_their_record_references() {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let prior = server.body_workload.as_ref().unwrap().evidence().clone();
    let sequence = prior.records.last().unwrap().sequence + 1;
    let (body, wake) = prior.body.wake(1, SignId::from("sign/woke")).unwrap();
    let lulled = wake.lull(SignId::from("sign/terminal")).unwrap();
    let failed = wake.fail(SignId::from("sign/terminal")).unwrap();
    let mut local = prior.clone();
    let mut observer = prior;
    local.append_wake(body.clone(), lulled, sequence).unwrap();
    observer.append_wake(body, failed, sequence).unwrap();
    assert_eq!(local.records, observer.records);
    let candidate = admission_extension(&observer);
    candidate.validate().unwrap();
    assert!(reconciliation::merge_membership_extension(&local, &candidate).is_err());
}
