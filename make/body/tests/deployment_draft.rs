use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_body::{BodyState, ResidentPlot};
use conduit_body_make::*;
use conduit_face::ApplicationEventKind;
use conduit_host_make::{parse_host_configuration_conduit, SporeOutputKind};
use conduit_workspace_make::{catalog, package_set};
use std::collections::BTreeMap;

fn encounter(name: &str) -> DeploymentDraft {
    encounter_with_memory(name, 64 * 1024 * 1024)
}

fn encounter_with_memory(name: &str, heap_bytes: u64) -> DeploymentDraft {
    let mut config = parse_host_configuration_conduit(include_str!(
        "../../../targets/std/profiles/linux-computer.host.conduit"
    ))
    .unwrap();
    config.limits.heap_arena_bytes = heap_bytes;
    let description = BodyDescription {
        schema: BODY_DESCRIPTION_SCHEMA,
        name: "new-body".into(),
        body: BodyBindingTarget {
            id: "body:construction-intent".into(),
        },
        hosts: ["first", "second"]
            .into_iter()
            .map(|name| BodyHostDescription {
                name: name.into(),
                part: Some(format!("part:{name}")),
                configuration: "linux.host.conduit".into(),
                spore: SporeDescription {
                    join_mode: SporeJoinMode::Prejoined,
                    output: SporeOutputKind::NativeBundle,
                    invitation: None,
                },
                deployment: None,
            })
            .collect(),
        mask_imports: vec![],
        wardrobe: BodyWardrobeDescription::default(),
    };
    let checked = check_body_description(
        description,
        &BTreeMap::from([("linux.host.conduit".into(), config)]),
        &catalog(),
        &package_set(),
    )
    .unwrap();
    let birth = BirthDraft::new(
        "01234567-89ab-4cde-8fab-0123456789ab".into(),
        vec![BirthPlotChoice {
            title: "Notes".into(),
            search_text: "notes".into(),
            plot: ResidentPlot::new("source/notes".into(), "checked/notes".into()),
            refusal: None,
            selected: true,
        }],
    )
    .unwrap();
    let mut draft = DeploymentDraft::new(birth, checked, "first".into()).unwrap();
    draft
        .apply_birth_event(
            draft.revision(),
            "creche.name",
            ApplicationEventKind::Input,
            name,
        )
        .unwrap();
    draft
}

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
