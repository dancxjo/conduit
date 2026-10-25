use conduit_birth_plot::{BirthDraft, BirthDraftRefusal, BirthPlotChoice};
use conduit_body::{BodyBiographyEvidence, BodyLifecycleSession, BodyState, ResidentPlot};
use conduit_body_make::{PreparedBirthRefusal, PreparedBodyBirth};

fn draft() -> BirthDraft {
    BirthDraft::new(
        "01234567-89ab-4cde-8fab-0123456789ab".into(),
        ["sampling", "display"]
            .into_iter()
            .map(|name| BirthPlotChoice {
                title: name.into(),
                search_text: name.into(),
                plot: ResidentPlot::new(
                    format!("source/{name}").into(),
                    format!("checked/{name}").into(),
                ),
                refusal: None,
                selected: true,
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn edited_encounter_survives_media_serialization_without_a_live_host() {
    let mut draft = draft();
    draft
        .edit_name(draft.revision(), "Willow's workshop".into())
        .unwrap();
    draft.select(draft.revision(), 1, false).unwrap();
    let reviewed = draft.selection(draft.revision()).unwrap();
    let prepared = PreparedBodyBirth::from_reviewed_draft(
        &draft,
        reviewed.revision,
        1,
        "sign/construction/birth".into(),
    )
    .unwrap();
    let encoded = serde_json::to_vec(prepared.evidence()).unwrap();
    // Two target builders consume one birth, not independently generated names/identities.
    let first: BodyBiographyEvidence = serde_json::from_slice(&encoded).unwrap();
    let second: BodyBiographyEvidence = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(first.body_id, second.body_id);
    assert_eq!(first.friendly_name, "Willow's workshop");
    assert_eq!(first.body.workset, reviewed.workset);
    assert_eq!(first.body.state, BodyState::Lulled);
    assert!(first.membership.parts.is_empty());
    let reopened = BodyLifecycleSession::open(first).unwrap();
    assert!(reopened.realization().is_none());
}

#[test]
fn changed_encounter_refuses_obsolete_review() {
    let mut draft = draft();
    let reviewed = draft.revision();
    draft.edit_name(reviewed, "Changed".into()).unwrap();
    assert!(matches!(
        PreparedBodyBirth::from_reviewed_draft(&draft, reviewed, 1, "sign/birth".into()),
        Err(PreparedBirthRefusal::Draft(BirthDraftRefusal::StaleFace))
    ));
}

#[test]
fn independent_births_keep_distinct_identities_despite_matching_names() {
    let draft = draft();
    let first = PreparedBodyBirth::from_reviewed_draft(
        &draft,
        draft.revision(),
        1,
        "sign/birth/one".into(),
    )
    .unwrap();
    let second = PreparedBodyBirth::from_reviewed_draft(
        &draft,
        draft.revision(),
        1,
        "sign/birth/two".into(),
    )
    .unwrap();
    assert_eq!(
        first.evidence().friendly_name,
        second.evidence().friendly_name
    );
    assert_ne!(first.evidence().body_id, second.evidence().body_id);
}

#[test]
fn unfinished_name_cannot_become_image_birth() {
    let mut draft = draft();
    draft.edit_name(draft.revision(), "   ".into()).unwrap();
    assert!(matches!(
        PreparedBodyBirth::from_reviewed_draft(&draft, draft.revision(), 1, "sign/birth".into()),
        Err(PreparedBirthRefusal::Draft(BirthDraftRefusal::InvalidName))
    ));
}
