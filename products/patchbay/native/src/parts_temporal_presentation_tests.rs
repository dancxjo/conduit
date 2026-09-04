use super::browser_presence::BrowserPresenceCoordinator;
use super::{browser_parts::BrowserPartsCoordinator, Arguments, PatchbayApplication};
use conduit_body::Body;
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use std::time::Duration;

#[test]
fn later_presentation_reference_advances_without_changing_the_presence_basis() {
    let body = Body::born(
        SourceDocumentId::from("source/native-temporal"),
        CheckedFormId::from("checked/native-temporal"),
        1,
        SignId::from("sign/native-temporal-born"),
    )
    .unwrap();
    let presence = BrowserPresenceCoordinator::new(body.body_id).unwrap();
    let first = presence.presentation_reference().unwrap();
    std::thread::sleep(Duration::from_millis(2));
    let later = presence.presentation_reference().unwrap();

    assert_eq!(later.identity, first.identity);
    assert_eq!(later.instant.clock_basis, first.instant.clock_basis);
    assert_eq!(later.instant.scale, first.instant.scale);
    assert_eq!(
        later.instant.resolution_ticks,
        first.instant.resolution_ticks
    );
    assert_eq!(
        later.instant.uncertainty_ticks,
        first.instant.uncertainty_ticks
    );
    assert!(later.instant.ticks > first.instant.ticks);
}

#[test]
fn live_parts_linear_presentation_orders_age_before_exact_provenance() {
    let directory = std::env::temp_dir().join(format!(
        "patchbay-native-parts-temporal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../../forms/hello/main.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    application.birth_body().unwrap();
    application.parts_open = true;
    application.linear_view = true;

    let body_id = application.build_birth.body().unwrap().body_id.clone();
    let part_id = application.build_birth.membership().unwrap().parts[0]
        .part_id
        .clone();
    let mut browser_parts = BrowserPartsCoordinator::new("page".into(), "chat".into());
    let sign = browser_parts
        .observe_for_presentation_test(
            &body_id,
            application.build_birth.membership().unwrap(),
            &part_id,
        )
        .unwrap();
    application.browser_parts = Some(browser_parts);

    let first = application.parts_portable_presentation().unwrap().unwrap();
    // Source and reference each declare one millisecond of uncertainty. Keep
    // their intervals disjoint so this proof deterministically exercises Past.
    std::thread::sleep(Duration::from_millis(4));
    let later = application.parts_portable_presentation().unwrap().unwrap();
    assert_eq!(
        first.temporal_facts[0].source,
        later.temporal_facts[0].source
    );
    assert_eq!(first.temporal_facts[0].sign_id, Some(sign));
    assert_eq!(
        first.temporal_facts[0].sign_id,
        later.temporal_facts[0].sign_id
    );
    assert!(
        later.temporal_references[0].instant.ticks > first.temporal_references[0].instant.ticks
    );

    let lines = application.presentation_lines();
    let relative = lines
        .iter()
        .position(|line| line.starts_with("RELATIVE_TIME "))
        .unwrap();
    let exact = lines
        .iter()
        .position(|line| line.starts_with("TEMPORAL_FACT "))
        .unwrap();
    assert!(relative < exact);

    let mut navigation =
        patchbay_model::PatchbayNavigationProjection::for_embodied(&later).unwrap();
    let body_place = navigation
        .navigation
        .places
        .iter()
        .find(|place| place.place == conduit_presentation::PresentationPlace::Body)
        .unwrap();
    navigation.cursor.place = body_place.place;
    navigation.cursor.aspect = body_place.aspects[0].aspect;
    navigation.cursor.focus = Some(later.temporal_facts[0].subject.clone());
    let ordinary =
        super::presentation::ordinary_front_door_lines(&later, &navigation, None).unwrap();
    assert!(ordinary.iter().any(|line| {
        line.contains("ago") && line.contains("Observation") && line.contains("exact time in F2")
    }));

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
