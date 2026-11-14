//! A selected owner leaves the library once; unrelated owners remain usable.
#[path = "publication.rs"]
mod publication;
pub use publication::decode;
#[path = "library_profiles.rs"]
mod profiles;

pub fn run(assignment: &crate::support::Assignment) -> crate::library::entry_0::Back {
    use conduit_body::{FederationPeer, HostCommandReceiveBounds, IssuedHostCommandChallenge};
    use conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal as E;
    use ed25519_dalek::{Signer, SigningKey};
    let active = assignment.active();
    let member: conduit_body::MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id":"fixture/credential","body_id":"fixture/body","part_id":"fixture/part",
        "host_id":"fixture/resident-cpu","boot_id":"fixture/boot/1","issued_at_millis":1,
    }))
    .unwrap();
    // Independent receiver identity/peer pin, not decoded from the assignment.
    let actual = conduit_core::HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "fixture/resident-cpu".into(),
        boot_id: "fixture/boot/1".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "fixture/resident-cpu".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let key = SigningKey::from_bytes(&[9; 32]);
    let issuer = FederationPeer {
        host_id: "fixture/issuer".into(),
        boot_id: "fixture/issuer/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        verifying_key: key.verifying_key().to_bytes(),
    };
    let (mut library, cost) =
        crate::probe::observe(|| crate::library::Initialized::prepare().unwrap());
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(
        library.computation_requirement_0(0).unwrap(),
        assignment.pins[0]
    );
    publication::check(library.offers());
    profiles::run(&mut library, assignment);
    let original = library.offers();
    let original = original.map(|offer| {
        let offer = offer.unwrap();
        (offer.as_ptr(), offer.len())
    });
    // A prepared selection keeps exclusive custody but neither withdraws an
    // offer nor retains the original assignment/Play inspection borrows.
    let transient = assignment.active();
    let (prepared, cost) = crate::probe::observe(|| {
        library
            .prepare_selected_array([Some(assignment.selected(&transient)), None])
            .unwrap()
    });
    drop(transient); // The opaque ticket retains no assignment/Play inspection.
    assert!(core::mem::size_of_val(&prepared) <= 3 * core::mem::size_of::<usize>());
    drop(prepared);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(
        library.offers().map(|o| {
            let o = o.unwrap();
            (o.as_ptr(), o.len())
        }),
        original
    );
    let ((), cost) = crate::probe::observe(|| {
        assert!(library
            .prepare_selected_array([
                Some(assignment.selected(&active)),
                Some(assignment.selected(&active)),
            ])
            .is_err());
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(
        library.offers().map(|o| {
            let o = o.unwrap();
            (o.as_ptr(), o.len())
        }),
        original
    );
    let ((), cost) = crate::probe::observe(|| {
        let prepared = library
            .prepare_selected_array([Some(assignment.selected(&active)), None])
            .unwrap();
        let [Some((node, mut back)), None] = prepared.bind() else {
            panic!("exact selected loan")
        };
        assert_eq!(node, 0);
        // This loan was never installed in a kernel or started. Explicitly end
        // its stateless scratch before returning the original availability.
        conduit_kernel::scheduler::StepBack::<1>::cancel(&mut back);
        assert!(back.finish_retired().is_ok());
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(
        library.offers().map(|o| {
            let o = o.unwrap();
            (o.as_ptr(), o.len())
        }),
        original
    );
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
        let slot = IssuedHostCommandChallenge::issue(
            &member,
            &actual,
            &issuer,
            HostCommandReceiveBounds {
                payload_contract: "fixture/assigned-fragment@1",
                maximum_payload_bytes: 8192,
                lifetime_millis: 10,
            },
            [1; 32],
            10,
        )
        .unwrap();
        let transcript = conduit_body::host_command_attribution_transcript(
            slot.challenge(),
            &assignment.encoded,
        )
        .unwrap();
        let signature = key.sign(&transcript).to_bytes();
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
        let attributed = slot.authenticate(bytes, &signature, 11).unwrap();
        let mut selected = assignment.selected(&active);
        selected.input = AssignedOperationInput::Contiguous(attributed.payload());
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
    publication::check(library.offers());
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
