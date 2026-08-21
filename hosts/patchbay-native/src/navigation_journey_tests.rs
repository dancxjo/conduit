use conduit_presentation::{
    enact_navigation_journey, render_linear_navigation, NavigationJourneyDisposition,
    NavigationOperation, NavigationRefusal, PresentationAspect, PresentationDepth,
    PresentationPlace, PresentationProjection,
};

#[test]
fn native_and_linear_consume_one_bounded_portable_navigation_journey() {
    let presentation = patchbay_model::portable_demonstration().unwrap();
    let mut navigation =
        patchbay_model::PatchbayNavigationProjection::for_embodied(&presentation).unwrap();
    let mut places = navigation.navigation.places.clone();
    places
        .iter_mut()
        .find(|place| place.place == PresentationPlace::Body)
        .unwrap()
        .aspects
        .retain(|aspect| aspect.aspect != PresentationAspect::Signs);
    let portable_navigation = conduit_presentation::PresentationNavigation::new(
        &presentation,
        places,
        navigation.navigation.follows.clone(),
    )
    .unwrap();
    let memberships = navigation
        .projection
        .memberships
        .iter()
        .filter(|membership| {
            membership.place != PresentationPlace::Body
                || membership.aspect != PresentationAspect::Signs
        })
        .cloned()
        .collect();
    navigation.projection =
        PresentationProjection::new(&presentation, &portable_navigation, memberships).unwrap();
    navigation.cursor.navigation = portable_navigation.identity.clone();
    navigation.navigation = portable_navigation;
    let follow = navigation.navigation.follows[0].clone();
    let operations = vec![
        NavigationOperation::Show(PresentationAspect::Plan),
        NavigationOperation::Focus(follow.source_subject.clone()),
        NavigationOperation::Follow(follow.identity),
        NavigationOperation::Disclose(PresentationDepth::Exact),
        NavigationOperation::Show(PresentationAspect::Signs),
        NavigationOperation::Back,
        NavigationOperation::Back,
        NavigationOperation::Back,
        NavigationOperation::Back,
        NavigationOperation::Back,
    ];
    let journey = enact_navigation_journey(
        &presentation,
        &navigation.navigation,
        navigation.cursor.clone(),
        8,
        &operations,
    )
    .unwrap();

    journey
        .validate(&presentation, &navigation.navigation)
        .unwrap();
    assert_eq!(journey.semantic_basis, presentation.basis);
    assert!(journey.steps.iter().all(|step| {
        step.semantic_basis == presentation.basis
            && (step.disposition == NavigationJourneyDisposition::Advanced
                || step.before_cursor == step.after_cursor)
    }));
    assert_eq!(
        journey.steps[4].disposition,
        NavigationJourneyDisposition::Refused(NavigationRefusal::UnknownAspect)
    );
    assert_eq!(
        journey.steps[9].disposition,
        NavigationJourneyDisposition::Refused(NavigationRefusal::HistoryExhausted)
    );

    let linear = journey
        .steps
        .iter()
        .filter(|step| step.disposition == NavigationJourneyDisposition::Advanced)
        .map(|step| {
            render_linear_navigation(
                &presentation,
                &navigation.navigation,
                &navigation.projection,
                &step.after_cursor,
            )
            .unwrap()
            .lines
        })
        .collect::<Vec<_>>();
    let native = crate::navigation_journey::portable_navigation_journey_lines(
        &presentation,
        &navigation,
        &journey,
    )
    .unwrap();
    assert_eq!(native.len(), linear.len());
    assert_ne!(native, linear);
    assert!(linear.iter().any(|lines| lines.iter().any(|line| {
        line.contains("place=Body") && line.contains("focus=") && line.contains("depth=Exact")
    })));
    assert!(native
        .iter()
        .any(|lines| lines.iter().any(|line| line.contains("PLACE Body"))
            && lines.iter().any(|line| line.contains("DEPTH Exact"))));

    let mut tampered = journey.clone();
    tampered.steps[0].after_cursor.depth = PresentationDepth::Exact;
    assert_eq!(
        tampered.validate(&presentation, &navigation.navigation),
        Err(NavigationRefusal::InvalidTruth)
    );
    assert_eq!(
        enact_navigation_journey(
            &presentation,
            &navigation.navigation,
            navigation.cursor,
            8,
            &vec![NavigationOperation::Back; 17],
        ),
        Err(NavigationRefusal::InvalidTruth)
    );
}
