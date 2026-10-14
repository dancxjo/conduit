use conduit_core::{BootId, HostId, SignId};
use conduit_face::{FaceAspect, FaceDepth, FacePlace, FaceRole, ProjectionItem};

use crate::{PlotCandidate, ZeroBodyFrontDoor};

fn explicit_seed(label: &str, source_name: &str, freshness: u64) -> PlotCandidate {
    PlotCandidate::from_source(
        label,
        source_name,
        include_str!("../../../../../plots/text-lab/main.conduit"),
        "portable navigation test source",
        SignId::from(format!("test/navigation/{freshness}")),
        freshness,
    )
    .unwrap()
}

fn projected_roles(
    projection: &crate::ZeroBodyFrontDoorProjection,
    place: FacePlace,
) -> Vec<FaceRole> {
    let mut cursor = projection.navigation.cursor.clone();
    cursor.place = place;
    cursor.aspect = FaceAspect::Structure;
    cursor.focus = None;
    cursor.depth = FaceDepth::Primary;
    projection
        .navigation
        .projection
        .project(&projection.face, &projection.navigation.navigation, &cursor)
        .unwrap()
        .items
        .iter()
        .filter_map(|membership| match &membership.item {
            ProjectionItem::Subject(identity) => projection
                .face
                .subjects
                .iter()
                .find(|subject| &subject.identity == identity)
                .map(|subject| subject.role.clone()),
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
        .add_plot(explicit_seed("Second", "second.conduit", 2))
        .unwrap();
    let initial = session.project().unwrap();
    assert_eq!(initial.navigation.cursor.place, FacePlace::Entrance);
    assert_eq!(initial.navigation.navigation.places.len(), 1);
    let seeds = initial
        .face
        .subjects
        .iter()
        .filter(|subject| subject.role == FaceRole::Plot)
        .map(|subject| subject.identity.clone())
        .collect::<Vec<_>>();
    assert_eq!(seeds.len(), 3);

    session
        .open_subject(&seeds[0], initial.face.revision)
        .unwrap();
    let opened = session.project().unwrap();
    assert!(opened.face.basis.body_id.is_none());
    assert_eq!(opened.navigation.cursor.place, FacePlace::Program);
    assert_eq!(opened.navigation.navigation.places.len(), 2);
    let entrance = projected_roles(&opened, FacePlace::Entrance);
    let program = projected_roles(&opened, FacePlace::Program);
    assert_eq!(
        entrance
            .iter()
            .filter(|role| **role == FaceRole::Plot)
            .count(),
        seeds.len()
    );
    assert_eq!(
        program
            .iter()
            .filter(|role| **role == FaceRole::Plot)
            .count(),
        1
    );
    assert_eq!(
        opened
            .face
            .disclosures
            .iter()
            .filter(|disclosure| {
                opened.face.subjects.iter().any(|subject| {
                    subject.identity == disclosure.subject && subject.role == FaceRole::Plot
                }) && disclosure.level == conduit_face::FaceDisclosureLevel::Primary
            })
            .count(),
        1
    );
    assert!(program.contains(&FaceRole::Plot));
    assert!(program.contains(&FaceRole::Gear));
    assert!(!program.contains(&FaceRole::Host));
    assert!(!program.contains(&FaceRole::Body));
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
        .face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Plot)
        .unwrap()
        .identity
        .clone();
    session.open_subject(&seed, initial.face.revision).unwrap();
    let opened = session.project().unwrap();
    let embodied = session.birth(opened.face.revision).unwrap();
    let projected = embodied.project().unwrap();

    assert_eq!(
        projected
            .navigation
            .navigation
            .places
            .iter()
            .map(|place| place.place)
            .collect::<Vec<_>>(),
        vec![FacePlace::Body, FacePlace::Program]
    );
    assert_eq!(projected.navigation.cursor.place, FacePlace::Body);
    let body = projected
        .navigation
        .projection
        .project(
            &projected.face,
            &projected.navigation.navigation,
            &projected.navigation.cursor,
        )
        .unwrap();
    assert!(body.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.face.subjects.iter().any(|subject| subject.identity == *identity && subject.role == FaceRole::Body))
    }));
    assert!(body.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.face.subjects.iter().any(|subject| subject.identity == *identity && subject.role == FaceRole::Plot))
    }));
    assert!(!body.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.face.subjects.iter().any(|subject| subject.identity == *identity && matches!(subject.role, FaceRole::Gear | FaceRole::Port | FaceRole::Cord)))
    }));

    let mut plot_cursor = projected.navigation.cursor.clone();
    plot_cursor.place = FacePlace::Program;
    plot_cursor.focus = projected
        .navigation
        .navigation
        .places
        .iter()
        .find(|place| place.place == FacePlace::Program)
        .map(|place| place.root_subject.clone());
    let plot = projected
        .navigation
        .projection
        .project(
            &projected.face,
            &projected.navigation.navigation,
            &plot_cursor,
        )
        .unwrap();
    assert!(plot.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.face.subjects.iter().any(|subject| subject.identity == *identity && subject.role == FaceRole::Gear))
    }));
    assert!(!plot.items.iter().any(|item| {
        matches!(&item.item, ProjectionItem::Subject(identity) if projected.face.subjects.iter().any(|subject| subject.identity == *identity && matches!(subject.role, FaceRole::Body | FaceRole::Part | FaceRole::Host | FaceRole::Line)))
    }));
}
