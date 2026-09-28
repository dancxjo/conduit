use conduit_core::kind_id;
use conduit_presentation::{
    render_linear_presentation, Presentation, PresentationAction, PresentationActionAvailability,
    PresentationBasis, PresentationCompositionKind, PresentationCompositionRelation,
    PresentationDisclosureLevel, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Specimen {
    identity: String,
    presentation: SpecimenPresentation,
}

#[derive(Debug, Deserialize)]
struct SpecimenPresentation {
    subjects: Vec<SpecimenSubject>,
    relationships: Vec<SpecimenRelationship>,
    composition: Vec<SpecimenComposition>,
    actions: Vec<SpecimenAction>,
}

#[derive(Debug, Deserialize)]
struct SpecimenSubject {
    identity: String,
    name: String,
    role: String,
}

#[derive(Debug, Deserialize)]
struct SpecimenRelationship {
    source: String,
    kind: String,
    target: String,
}

#[derive(Debug, Deserialize)]
struct SpecimenComposition {
    source: String,
    kind: String,
    target: String,
}

#[derive(Debug, Deserialize)]
struct SpecimenAction {
    identity: String,
    intent: String,
    target: String,
    name: String,
    availability: String,
}

fn fixtures() -> Vec<Specimen> {
    serde_json::from_str(include_str!(
        "../../../proof/conformance/presentation-waist/specimens.json"
    ))
    .expect("shared Presentation-waist fixtures must remain valid JSON")
}

fn relationship_kind(kind: &str) -> PresentationRelationshipKind {
    match kind {
        "Contains" => PresentationRelationshipKind::Contains,
        "Connects" => PresentationRelationshipKind::Connects,
        "Describes" => PresentationRelationshipKind::Describes,
        "Realizes" => PresentationRelationshipKind::Realizes,
        "Observes" => PresentationRelationshipKind::Observes,
        semantic => PresentationRelationshipKind::Semantic(kind_id(semantic)),
    }
}

fn composition_kind(kind: &str) -> PresentationCompositionKind {
    match kind {
        "Group" => PresentationCompositionKind::Group,
        "Contrast" => PresentationCompositionKind::Contrast,
        "Juxtapose" => PresentationCompositionKind::Juxtapose,
        "Emphasize" => PresentationCompositionKind::Emphasize,
        "Subordinate" => PresentationCompositionKind::Subordinate,
        "Associate" => PresentationCompositionKind::Associate,
        "RevealAfter" => PresentationCompositionKind::RevealAfter,
        semantic => PresentationCompositionKind::Semantic(kind_id(semantic)),
    }
}

fn presentation(specimen: &Specimen) -> Presentation {
    let value = &specimen.presentation;
    Presentation::new_with_semantics(
        1,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        value
            .subjects
            .iter()
            .map(|subject| PresentationSubject {
                identity: subject.identity.clone(),
                role: PresentationRole::Semantic(kind_id(&subject.role)),
                name: subject.name.clone(),
            })
            .collect(),
        value
            .relationships
            .iter()
            .map(|relationship| PresentationRelationship {
                source: relationship.source.clone(),
                target: relationship.target.clone(),
                kind: relationship_kind(&relationship.kind),
            })
            .collect(),
        vec![],
        vec![],
        value
            .actions
            .iter()
            .map(|action| {
                assert_eq!(action.availability, "available");
                PresentationAction {
                    identity: action.identity.clone(),
                    intent: action.intent.clone(),
                    target: action.target.clone(),
                    name: action.name.clone(),
                    disclosure: PresentationDisclosureLevel::CurrentAction,
                    availability: PresentationActionAvailability::Available,
                }
            })
            .collect(),
        vec![],
    )
    .expect("specimen must construct an ordinary Presentation")
    .with_composition(
        value
            .composition
            .iter()
            .enumerate()
            .map(|(index, relation)| PresentationCompositionRelation {
                identity: format!("composition/{}/{index}", specimen.identity),
                source: relation.source.clone(),
                target: relation.target.clone(),
                kind: composition_kind(&relation.kind),
            })
            .collect(),
    )
    .expect("specimen composition must remain renderer-neutral and valid")
}

#[test]
fn all_adversarial_specimens_are_valid_current_presentations() {
    let specimens = fixtures();
    assert_eq!(specimens.len(), 10);

    for specimen in &specimens {
        let presentation = presentation(specimen);
        presentation
            .validate()
            .unwrap_or_else(|error| panic!("{} did not validate: {error:?}", specimen.identity));
        assert_eq!(
            presentation.subjects.len(),
            specimen.presentation.subjects.len()
        );
        assert_eq!(
            presentation.relationships.len(),
            specimen.presentation.relationships.len()
        );
        assert_eq!(
            presentation.composition.len(),
            specimen.presentation.composition.len()
        );
        assert_eq!(
            presentation.actions.len(),
            specimen.presentation.actions.len()
        );
    }
}

#[test]
fn deterministic_linear_projection_preserves_every_specimen_record() {
    for specimen in fixtures() {
        let presentation = presentation(&specimen);
        let linear = render_linear_presentation(&presentation)
            .unwrap_or_else(|error| panic!("{} did not render: {error:?}", specimen.identity));

        for subject in &specimen.presentation.subjects {
            assert!(
                linear.lines.iter().any(|line| {
                    line.starts_with("SUBJECT ")
                        && line.contains(&format!("id={:?}", subject.identity))
                        && line.contains(&format!("name={:?}", subject.name))
                }),
                "{} lost subject {}",
                specimen.identity,
                subject.identity
            );
        }
        for action in &specimen.presentation.actions {
            assert!(
                linear.lines.iter().any(|line| {
                    line.starts_with("ACTION ")
                        && line.contains(&format!("id={:?}", action.identity))
                        && line.contains(&format!("intent={:?}", action.intent))
                }),
                "{} lost action {}",
                specimen.identity,
                action.identity
            );
        }
        assert_eq!(
            linear
                .lines
                .iter()
                .filter(|line| line.starts_with("RELATIONSHIP "))
                .count(),
            specimen.presentation.relationships.len(),
            "{} lost relationship truth",
            specimen.identity
        );
        assert_eq!(
            linear
                .lines
                .iter()
                .filter(|line| line.starts_with("COMPOSITION "))
                .count(),
            specimen.presentation.composition.len(),
            "{} lost encounter structure",
            specimen.identity
        );
    }
}
