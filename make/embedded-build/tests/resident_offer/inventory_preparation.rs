//! Execute the actual Root's read-only preparer against emitted initialized code.
use super::*;
use sha2::Digest;
fn signed_document(metadata: &BodyFragmentApprovalMetadata<'_>, projection: &[u8]) -> Vec<u8> {
    encode_body_fragment_document(metadata, projection, 8192).unwrap()
}
pub fn run(scope: PreparedBodyFragment<'_>, a: &crate::support::Assignment) {
    let document = document(scope, a);
    let receiver = Receiver::new(scope);
    let mut library = crate::library::Initialized::prepare().unwrap();
    let images = library.offers().map(|image| image.unwrap().as_ptr());
    let identity = library.available_identity(0).unwrap() as *const _;
    let (result, cost) = crate::probe::observe(|| {
        crate::root_preparer::validate_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command(document.bytes()),
            a.maxima,
        )
    });
    assert_eq!(result, Ok(()));
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(
        library.offers().map(|image| image.unwrap().as_ptr()),
        images
    );
    assert_eq!(library.available_identity(0).unwrap() as *const _, identity);
    let selected = library.identify_assignments(&a.encoded).unwrap();
    assert_eq!(
        selected.nodes().collect::<Vec<_>>(),
        vec![
            conduit_plan_lowering::resident_inventory::ResidentNodeSelection { node: 0, entry: 0 }
        ]
    );
    assert_eq!(selected.computations(), a.pins);
    mixed_inventory(&library, a);
    let mut no_fore = profile(a);
    no_fore.fore = None;
    let ((fragment, fore), cost) = crate::probe::observe(|| {
        receive_delegated_body_fragment_with_approved_fore::<2>(
            Some(&policy()),
            receiver.context(),
            receiver.command(document.bytes()),
            no_fore,
        )
        .unwrap()
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(fore.as_slice(), a.fore);
    bounded_tables::run(&fragment, a, &mut library);
    assert_eq!(
        fragment.view(),
        c::decode_assigned_plan_with_fore(
            &a.encoded,
            a.maxima,
            profile(a).requirements,
            profile(a).computations,
            &a.fore
        )
        .unwrap()
    );
    let mut no_fore = profile(a);
    no_fore.fore = None;
    assert!(matches!(
        receive_delegated_body_fragment_with_approved_fore::<1>(
            Some(&policy()),
            receiver.context(),
            receiver.command(document.bytes()),
            no_fore
        ),
        Err(DelegatedFragmentRefusal::ForeApproval)
    ));
    assert!(matches!(
        receive_delegated_body_fragment_with_approved_fore::<2>(
            Some(&policy()),
            receiver.context(),
            receiver.command(document.bytes()),
            profile(a)
        ),
        Err(DelegatedFragmentRefusal::ForeApproval)
    )); // Do not override existing local pins.
    assert_eq!(
        crate::root_preparer::validate_with_inventory(
            &library,
            None,
            receiver.context(),
            receiver.command(document.bytes()),
            a.maxima
        ),
        Err(crate::root_preparer::ValidationRefusal::Policy)
    );
    let mut small = a.maxima;
    small.runtime_state_bytes = 1;
    assert_eq!(
        crate::root_preparer::validate_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command(document.bytes()),
            small
        ),
        Err(crate::root_preparer::ValidationRefusal::Assignment)
    );
    let (metadata, original) = decode_body_fragment_document(document.bytes()).unwrap();
    for mutation in 0..3 {
        let mut projection = original.to_vec();
        let header = c::ASSIGNED_PLAN_HEADER_BYTES;
        match mutation {
            0 => projection[header + 3 + 36] ^= 1, // A different artifact is not installed.
            1 => projection[header + 3 + 2] ^= 1,  // Same identity cannot change the work grant.
            _ => projection[header + 3..header + 5].copy_from_slice(&2u16.to_le_bytes()),
        }
        let digest = c::assigned_plan_payload_digest(&projection[header..]);
        projection[93..125].copy_from_slice(&digest);
        let mut metadata = metadata;
        metadata.projection_digest = sha2::Sha256::digest(&projection).into();
        let bytes = signed_document(&metadata, &projection);
        let (result, cost) = crate::probe::observe(|| {
            crate::root_preparer::validate_with_inventory(
                &library,
                Some(&policy()),
                receiver.context(),
                receiver.command(&bytes),
                a.maxima,
            )
        });
        assert!(result.is_err());
        assert_eq!(cost.allocations, 0);
        assert!(library.offers().iter().all(Option::is_some));
    }
    let mut corrupt = metadata.fore_ports.to_vec();
    corrupt[21] ^= 1;
    let mut metadata = metadata;
    metadata.fore_ports = &corrupt;
    let bytes = signed_document(&metadata, original);
    assert_eq!(
        crate::root_preparer::validate_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command(&bytes),
            a.maxima
        ),
        Err(crate::root_preparer::ValidationRefusal::Assignment)
    );
    let (_, loan) = library
        .borrow_entry_0(a.selected_scope(AssignedPlayScope::Delegated(&fragment)))
        .unwrap();
    drop(loan); // Explicitly quarantine an abandoned execution owner.
    assert!(library.available_identity(0).is_none());
    assert!(library.available_identity(1).is_some());
    assert_eq!(
        crate::root_preparer::validate_with_inventory(
            &library,
            Some(&policy()),
            receiver.context(),
            receiver.command(document.bytes()),
            a.maxima
        ),
        Err(crate::root_preparer::ValidationRefusal::Selection)
    );
    println!("actual Root inventory/profile preflight: real initialized identity pins, approved exact Fore ABI, zero allocation or provider transfer; unavailable/mutated artifact, fuel, node, Fore and insufficient memory refused");
}

#[path = "bounded_tables.rs"]
mod bounded_tables;


fn mixed_inventory(library: &crate::library::Initialized, a: &crate::support::Assignment) {
    use conduit_plan_lowering::resident_inventory::OtherNodeIdentity;
    let pin = OtherNodeIdentity {
        node: 1,
        kind: c::AssignedIdentity::from_text("fixture/native-kind"),
        implementation: c::AssignedIdentity::from_text("fixture/native-implementation"),
        artifact: c::AssignedIdentity::from_text("fixture/native-artifact"),
    };
    // Deliberately only a node-identity inspection envelope. The synthetic
    // native node has no Host Call, resource or authority and cannot execute.
    let mut bytes = a.encoded.clone();
    bytes.push(c::ASSIGNED_NODE);
    bytes.extend_from_slice(&52u16.to_le_bytes());
    bytes.extend_from_slice(&pin.node.to_le_bytes());
    bytes.extend_from_slice(&[0; 2]);
    bytes.extend_from_slice(&pin.kind.0);
    bytes.extend_from_slice(&pin.implementation.0);
    bytes.extend_from_slice(&pin.artifact.0);
    let length = bytes.len() as u16;
    bytes[10..12].copy_from_slice(&length.to_le_bytes());
    assert!(library.identify_assignments(&bytes).is_err());
    let (selected, cost) = crate::probe::observe(|| library.identify_mixed_assignments(&bytes, &[pin]).unwrap());
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(selected.computations(), a.pins);
    assert_eq!(selected.nodes().next().unwrap().entry, 0);
    assert!(library.offers().iter().all(Option::is_some));
}
