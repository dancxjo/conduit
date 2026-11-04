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
    let ((node, back), cost) = crate::probe::observe(|| {
        use conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput;
        use conduit_wire::{encode_bounded_document_chunk, BorrowedBoundedDocumentAssembly};
        use sha2::{Digest, Sha256};
        // CPU receive-workspace fixture, independent of encoded/runtime grants.
        let mut storage = [0; 8192];
        let mut frame = [0; 128];
        let mut receiver = BorrowedBoundedDocumentAssembly::new(&mut storage, 16);
        let digest: [u8; 32] = Sha256::digest(&assignment.encoded).into();
        let mut offset = 0;
        let mut chunks = 0;
        while offset < assignment.encoded.len() {
            let (length, next) =
                encode_bounded_document_chunk(&mut frame, &assignment.encoded, offset, &digest)
                    .unwrap();
            let complete = receiver.push(&frame[..length]).unwrap();
            assert_eq!(complete, next == assignment.encoded.len());
            offset = next;
            chunks += 1;
        }
        assert!(chunks > 1);
        let (bytes, expected) = receiver.completed().unwrap();
        assert_eq!(Sha256::digest(bytes).as_slice(), expected);
        let mut selected = assignment.selected(&active);
        selected.input = AssignedOperationInput::Contiguous(bytes);
        library.bind_entry_0(selected).unwrap()
    });
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
