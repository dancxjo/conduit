use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership, MembershipProofId,
    PartId,
};
use conduit_core::{OfferGeneration, SignId};

fn checked(source: &str) -> ResidentPlot {
    let (catalog, _) = crate::installed_browser::catalogs_for_presentation(
        crate::installed_browser::PresentationProfile::Annotation,
    )
    .unwrap();
    let syntax = conduit_plot::parse_syntax_document(source);
    let checked = conduit_plot::check_syntax_document(&syntax, &catalog).unwrap();
    ResidentPlot::new(
        checked.source_document_id.clone(),
        checked.plots[0].checked_plot_id.clone(),
    )
}

fn session(plot: &ResidentPlot) -> (BodyLifecycleSession, HostId, BootId) {
    let host = HostId::from("host/replace-test");
    let boot = BootId::from("boot/replace-test");
    let body = Body::born(
        plot.source_document_id.clone(),
        plot.checked_plot_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Workset".into()).unwrap();
    let part = PartId::bind(&body.body_id, "browser", 1).unwrap();
    let proof = MembershipProofId::bind("proof/replace-test").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from("sign/admitted"),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            SignId::from("sign/present"),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    (BodyLifecycleSession::open(evidence).unwrap(), host, boot)
}

const BEFORE: &str = "plot clock {\n ticks: time/every(1s)\n ticks >> presentation/tick\n}";
const AFTER: &str = "plot clock {\n ticks: time/every(2s)\n ticks >> presentation/tick\n}";

#[test]
fn replace_admits_new_identity_and_selects_it_without_mutating_original() {
    let old = checked(BEFORE);
    let next = checked(AFTER);
    let (original, host, boot) = session(&old);
    let revision = original.evidence().body.workload_revision;
    let updated = change(
        &original,
        &host,
        &boot,
        revision,
        next.clone(),
        AFTER,
        WorksetEdit::Replace {
            previous: old.clone(),
        },
    )
    .unwrap_or_else(|_| panic!("valid atomic replacement refused"));
    assert_eq!(updated.foreground(), Some(&next));
    assert_eq!(updated.evidence().body.workload_revision, revision + 2);
    assert!(!updated.evidence().body.workset.plots().contains(&old));
    assert!(original.evidence().body.workset.plots().contains(&old));
}

#[test]
fn rejected_new_source_or_stale_revision_leaves_original_unchanged() {
    let old = checked(BEFORE);
    let next = checked(AFTER);
    let (original, host, boot) = session(&old);
    let before = serde_json::to_vec(original.evidence()).unwrap();
    let revision = original.evidence().body.workload_revision;
    for (source, revision) in [(BEFORE, revision), (AFTER, revision + 1)] {
        assert!(change(
            &original,
            &host,
            &boot,
            revision,
            next.clone(),
            source,
            WorksetEdit::Replace {
                previous: old.clone()
            }
        )
        .is_err());
        assert_eq!(serde_json::to_vec(original.evidence()).unwrap(), before);
    }
}

#[test]
fn refusal_after_candidate_removal_keeps_old_resident_and_revision() {
    let old = checked(BEFORE);
    let next = checked(AFTER);
    let (mut original, host, boot) = session(&old);
    original
        .admit_plot(
            original.evidence().body.workload_revision,
            next.clone(),
            &host,
            &boot,
        )
        .unwrap();
    let before = serde_json::to_vec(original.evidence()).unwrap();
    let revision = original.evidence().body.workload_revision;
    assert!(change(
        &original,
        &host,
        &boot,
        revision,
        next,
        AFTER,
        WorksetEdit::Replace { previous: old }
    )
    .is_err());
    assert_eq!(serde_json::to_vec(original.evidence()).unwrap(), before);
}

#[test]
fn replace_uses_exact_nested_previous_identity_in_json() {
    let previous = checked(BEFORE);
    let encoded = serde_json::json!({"Replace": {"previous": previous}});
    let WorksetEdit::Replace { previous: decoded } = serde_json::from_value(encoded).unwrap()
    else {
        panic!("replace edit");
    };
    assert_eq!(decoded, previous);
}
