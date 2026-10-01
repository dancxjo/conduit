use conduit_alife::{
    LeniaFieldId, ReactionDiffusionCell, ReactionDiffusionEvolveRequest, ReactionDiffusionFieldId,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn field_ids_are_exact_fixed_collections() {
    let reaction = ReactionDiffusionFieldId::from_bytes(*b"field-a0-proof01");
    assert_eq!(reaction.get(), b"field-a0-proof01");
    assert_eq!(
        ReactionDiffusionFieldId::from_structured(reaction.into_structured().unwrap()).unwrap(),
        reaction
    );

    let lenia = LeniaFieldId::from_bytes(*b"lenia-field-0001");
    assert_eq!(lenia.get(), b"lenia-field-0001");
    assert_eq!(
        LeniaFieldId::from_structured(lenia.into_structured().unwrap()).unwrap(),
        lenia
    );
}

#[test]
fn cells_and_evolve_requests_enforce_native_bounds() {
    for cell in [
        ReactionDiffusionCell::new_native(0, 0).unwrap(),
        ReactionDiffusionCell::new_native(1_000_000, 1_000_000).unwrap(),
    ] {
        assert_eq!(
            ReactionDiffusionCell::from_structured(cell.into_structured().unwrap()).unwrap(),
            cell
        );
    }
    assert!(ReactionDiffusionCell::new_native(1_000_001, 0).is_err());

    let field_id = ReactionDiffusionFieldId::from_bytes(*b"field-a0-proof01");
    for generations in [1, 64] {
        let request = ReactionDiffusionEvolveRequest::new(field_id, 7, generations, 4_096).unwrap();
        assert_eq!(
            ReactionDiffusionEvolveRequest::from_structured(request.into_structured().unwrap())
                .unwrap(),
            request
        );
    }
    assert!(ReactionDiffusionEvolveRequest::new(field_id, 7, 0, 4_096).is_err());
    assert!(ReactionDiffusionEvolveRequest::new(field_id, 7, 65, 4_096).is_err());
}
