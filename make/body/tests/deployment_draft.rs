use conduit_body::BodyState;
use conduit_body_make::*;
use conduit_face::ApplicationEventKind;
use conduit_workspace_make::{catalog, package_set};
#[path = "common/deployment.rs"]
mod fixture;
use fixture::{encounter, encounter_with_memory};

#[test]
fn all_host_preparation_binds_one_birth_to_checked_host_construction() {
    let mut draft = encounter("Signal Garden");
    draft
        .select_hosts(draft.revision(), "second", DeploymentScope::AllHosts)
        .unwrap();
    draft
        .select_image_source(draft.revision(), DeploymentImageSource::SourceBuild)
        .unwrap();
    let review = draft.review(draft.revision(), &package_set()).unwrap();
    let prepared = draft
        .prepare(
            review,
            1,
            "sign/birth/garden".into(),
            &catalog(),
            &package_set(),
        )
        .unwrap();
    assert_eq!(prepared.birth().evidence().friendly_name, "Signal Garden");
    assert_eq!(
        prepared.construction().description().body.id,
        prepared.birth().evidence().body_id.as_str()
    );
    assert_eq!(prepared.selected_hosts(), ["first", "second"]);
    assert_eq!(prepared.first_host(), "second");
    assert_eq!(prepared.image_source(), DeploymentImageSource::SourceBuild);
    assert_eq!(prepared.birth().evidence().body.state, BodyState::Lulled);
    assert!(prepared.birth().evidence().membership.parts.is_empty());
}

#[test]
fn default_prepares_only_the_explicit_first_host() {
    let draft = encounter("One Host");
    let review = draft.review(draft.revision(), &package_set()).unwrap();
    let prepared = draft
        .prepare(
            review,
            1,
            "sign/birth/one".into(),
            &catalog(),
            &package_set(),
        )
        .unwrap();
    assert_eq!(prepared.selected_hosts(), ["first"]);
    assert_eq!(prepared.construction().hosts().len(), 2);
}

#[test]
fn changes_after_review_refuse_before_preparation() {
    let mut draft = encounter("Old name");
    let review = draft.review(draft.revision(), &package_set()).unwrap();
    draft
        .apply_birth_event(
            draft.revision(),
            "creche.name",
            ApplicationEventKind::Input,
            "New name",
        )
        .unwrap();
    assert!(matches!(
        draft.prepare(
            review,
            1,
            "sign/birth/stale".into(),
            &catalog(),
            &package_set()
        ),
        Err(DeploymentDraftRefusal::StaleReview)
    ));
}

#[test]
fn review_from_a_different_name_at_the_same_revision_is_rejected() {
    let first = encounter("First name");
    let second = encounter("Other name");
    assert_eq!(first.revision(), second.revision());
    let review = first.review(first.revision(), &package_set()).unwrap();
    assert!(matches!(
        second.prepare(
            review,
            1,
            "sign/birth/wrong-review".into(),
            &catalog(),
            &package_set()
        ),
        Err(DeploymentDraftRefusal::StaleReview)
    ));
}

#[test]
fn unknown_host_and_direct_birth_do_not_mutate_the_encounter() {
    let mut draft = encounter("Garden");
    let revision = draft.revision();
    assert!(matches!(
        draft.select_hosts(revision, "missing", DeploymentScope::AllHosts),
        Err(DeploymentDraftRefusal::UnknownHost)
    ));
    assert!(matches!(
        draft.apply_birth_event(revision, "creche.birth", ApplicationEventKind::Activate, ""),
        Err(DeploymentDraftRefusal::BirthActionNotAvailable)
    ));
    assert_eq!(draft.revision(), revision);
    assert_eq!(draft.first_host(), "first");
    assert_eq!(draft.scope(), DeploymentScope::FirstHost);
}

#[test]
fn same_recipe_reference_with_different_profile_cannot_reuse_review() {
    let first = encounter("Garden");
    let second = encounter_with_memory("Garden", 32 * 1024 * 1024);
    assert_eq!(
        first.construction().description_id(),
        second.construction().description_id()
    );
    let review = first.review(first.revision(), &package_set()).unwrap();
    assert!(matches!(
        second.prepare(
            review,
            1,
            "sign/birth/wrong-profile".into(),
            &catalog(),
            &package_set()
        ),
        Err(DeploymentDraftRefusal::StaleReview)
    ));
}
