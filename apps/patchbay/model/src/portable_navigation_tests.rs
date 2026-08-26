use conduit_core::{BootId, HostId, SignId};
use conduit_presentation::{
    PresentationAspect, PresentationDepth, PresentationPlace, PresentationRole, ProjectionItem,
};

use crate::{SeedCandidate, ZeroBodyFrontDoor};

fn explicit_seed(label: &str, source_name: &str, freshness: u64) -> SeedCandidate {
    SeedCandidate::from_source(
        label,
        source_name,
        include_str!("../../../../examples/text-lab.conduit"),
        "portable navigation test source",
        SignId::from(format!("test/navigation/{freshness}")),
        freshness,
    )
    .unwrap()
}

fn projected_roles(
    projection: &crate::ZeroBodyFrontDoorProjection,
    place: PresentationPlace,
) -> Vec<PresentationRole> {
    let mut cursor = projection.navigation.cursor.clone();
    cursor.place = place;
    cursor.aspect = PresentationAspect::Structure;
    cursor.focus = None;
    cursor.depth = PresentationDepth::Primary;
    projection
        .navigation
        .projection
        .project(
            &projection.presentation,
            &projection.navigation.navigation,
            &cursor,
        )
        .unwrap()
        .items
        .iter()
        .filter_map(|membership| match &membership.item {
            ProjectionItem::Subject(identity) => projection
                .presentation
                .subjects
                .iter()
                .find(|subject| &subject.identity == identity)
                .map(|subject| subject.role),
            _ => None,
        })
        .collect()
}

#[test]
fn opening_a_seed_moves_the_default_cursor_from_entrance_to_program() {
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("navigation/host"),
        BootId::from("navigation/boot"),
    )
    .unwrap();
    session
        .add_seed(explicit_seed("Second", "second.conduit", 2))
        .unwrap();
    let initial = session.project().unwrap();
    assert_eq!(initial.navigation.cursor.place, PresentationPlace::Entrance);
    assert_eq!(initial.navigation.navigation.places.len(), 1);
    let seeds = initial
        .presentation
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Seed)
        .map(|subject| subject.identity.clone())
        .collect::<Vec<_>>();
    assert_eq!(seeds.len(), 2);

    session
        .open_subject(&seeds[0], initial.presentation.revision)
        .unwrap();
    let opened = session.project().unwrap();
    assert!(opened.presentation.basis.body_id.is_none());
    assert_eq!(opened.navigation.cursor.place, PresentationPlace::Program);
    assert_eq!(opened.navigation.navigation.places.len(), 2);
    let entrance = projected_roles(&opened, PresentationPlace::Entrance);
    let program = projected_roles(&opened, PresentationPlace::Program);
    assert_eq!(
        entrance
            .iter()
            .filter(|role| **role == PresentationRole::Seed)
            .count(),
        2
    );
    assert_eq!(
        program
            .iter()
            .filter(|role| **role == PresentationRole::Seed)
            .count(),
        1
    );
    assert_eq!(
        opened
            .presentation
            .disclosures
            .iter()
            .filter(|disclosure| {
                opened.presentation.subjects.iter().any(|subject| {
                    subject.identity == disclosure.subject && subject.role == PresentationRole::Seed
                }) && disclosure.level == conduit_presentation::PresentationDisclosureLevel::Primary
            })
            .count(),
        1
    );
    assert!(program.contains(&PresentationRole::Form));
    assert!(program.contains(&PresentationRole::Gear));
    assert!(!program.contains(&PresentationRole::Host));
    assert!(!program.contains(&PresentationRole::Body));
}

#[test]
fn birth_removes_entrance_and_keeps_program_and_body_subject_sets_distinct() {
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("embodied/host"),
        BootId::from("embodied/boot"),
    )
    .unwrap();
    let initial = session.project().unwrap();
    let seed = initial
        .presentation
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Seed)
        .unwrap()
        .identity
        .clone();
    session
        .open_subject(&seed, initial.presentation.revision)
        .unwrap();
    let opened = session.project().unwrap();
    let embodied = session.birth(opened.presentation.revision).unwrap();
    let projected = embodied.project().unwrap();

    assert_eq!(
        projected
            .navigation
            .navigation
            .places
            .iter()
            .map(|place| place.place)
            .collect::<Vec<_>>(),
        vec![PresentationPlace::Program, PresentationPlace::Body]
    );
    assert_eq!(
        projected.navigation.cursor.place,
        PresentationPlace::Program
    );
    let program = projected
        .navigation
        .projection
        .project(
            &projected.presentation,
            &projected.navigation.navigation,
            &projected.navigation.cursor,
        )
        .unwrap();
    assert!(program.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.presentation.subjects.iter().any(|subject| subject.identity == *identity && subject.role == PresentationRole::Gear))
    }));
    assert!(!program.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.presentation.subjects.iter().any(|subject| subject.identity == *identity && matches!(subject.role, PresentationRole::Body | PresentationRole::Part | PresentationRole::Host | PresentationRole::Line | PresentationRole::Seed)))
    }));

    let mut body_cursor = projected.navigation.cursor.clone();
    body_cursor.place = PresentationPlace::Body;
    body_cursor.focus = None;
    let body = projected
        .navigation
        .projection
        .project(
            &projected.presentation,
            &projected.navigation.navigation,
            &body_cursor,
        )
        .unwrap();
    assert!(body.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.presentation.subjects.iter().any(|subject| subject.identity == *identity && subject.role == PresentationRole::Body))
    }));
    assert!(!body.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.presentation.subjects.iter().any(|subject| subject.identity == *identity && matches!(subject.role, PresentationRole::Gear | PresentationRole::Port | PresentationRole::Cord | PresentationRole::Form)))
    }));
}
