//! A selected owner leaves the library once; unrelated owners remain usable.
pub fn run(assignment: &crate::support::Assignment) -> crate::library::entry_0::Back {
    use conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal as E;
    let active = assignment.active();
    let (mut library, cost) =
        crate::probe::observe(|| crate::library::Initialized::prepare().unwrap());
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(
        library.computation_requirement_0(0).unwrap(),
        assignment.pins[0]
    );
    let original = library.offers();
    let original = original.map(|offer| {
        let offer = offer.unwrap();
        (offer.as_ptr(), offer.len())
    });
    // The same program with a different temporal Kind is not interchangeable.
    let (result, cost) =
        crate::probe::observe(|| library.bind_entry_1(assignment.selected(&active)));
    assert!(matches!(result, Err(E::Selection)));
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(
        library.offers().map(|offer| {
            let offer = offer.unwrap();
            (offer.as_ptr(), offer.len())
        }),
        original
    );
    let mut stale = active.clone();
    stale.play_sequence += 1;
    assert!(matches!(
        library.bind_entry_0(assignment.selected(&stale)),
        Err(E::Scope)
    ));
    assert!(library.offers().iter().all(Option::is_some));
    let ((node, back), cost) =
        crate::probe::observe(|| library.bind_entry_0(assignment.selected(&active)).unwrap());
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(node, 0);
    assert!(back.required_step_fuel() <= assignment.tables.nodes[0].maximum_step_fuel);
    let offers = library.offers();
    assert!(offers[0].is_none());
    let other = offers[1].unwrap();
    assert_eq!((other.as_ptr(), other.len()), original[1]);
    assert!(matches!(
        library.bind_entry_0(assignment.selected(&active)),
        Err(E::MissingOwner)
    ));
    assert!(matches!(
        library.computation_requirement_0(0),
        Err(E::MissingOwner)
    ));
    assert!(library.computation_requirement_1(0).is_ok());
    let (taken, remaining) = library.into_backs();
    assert!(taken.is_none());
    assert!(remaining.unwrap().required_step_fuel() <= 1002);
    back
}
