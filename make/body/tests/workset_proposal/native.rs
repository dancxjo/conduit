//! The consumer uses actual Native custody, not the fixture's field decoder.
use super::*;
use conduit_plot::rust_binding::{
    PreparedNativeAdmission, PreparedNativeBounds, PreparedNativeProgress,
};
fn admission<'a>(
    document: &'a CheckedSyntaxDocument,
    name: &str,
    bytes: &[u8],
) -> PreparedNativeAdmission<'a> {
    let identity = &document
        .native_types
        .iter()
        .find(|native| native.name == name)
        .unwrap()
        .identity;
    let mut admission = PreparedNativeAdmission::new_with_occurrence_limit(
        document,
        identity,
        PreparedNativeBounds {
            canonical_bytes: MAX_WORKSET_PROPOSAL_BYTES,
            scopes: 64,
            invariants: 64,
            contracts: 64,
        },
        4096,
    )
    .unwrap();
    admission.begin(bytes).unwrap();
    while admission.step().unwrap() != PreparedNativeProgress::Admitted {}
    admission
}
#[test]
fn admitted_authored_info_normalizes_and_foreign_or_stale_basis_refuses() {
    use WorksetProposalRefusal as E;
    let a = resident(&report("first"));
    let checked = report("second");
    let b = resident(&checked);
    let observed = body(&a, 1);
    let before = observed.clone();
    let add = program("propose-add");
    let input = add_input(&add, &observed, &a, &checked);
    let bytes = add.evaluate(&input).unwrap();
    let declaration = declaration();
    let admitted = admission(&declaration, "WorksetProposalInfo", &bytes);
    let proof = admitted.admitted().unwrap();
    let expected = propose_checked_workset(
        &observed,
        &observed.body_id,
        0,
        vec![a.clone(), b.clone()],
        Some(candidate(&checked)),
    )
    .unwrap();
    assert_eq!(
        propose_admitted_workset(&observed, &declaration, &proof, Some(candidate(&checked)))
            .unwrap(),
        expected
    );
    let foreign = super::declaration();
    assert_eq!(foreign.source_document_id, declaration.source_document_id);
    assert_eq!(
        propose_admitted_workset(&observed, &foreign, &proof, Some(candidate(&checked))),
        Err(E::NativeOwner)
    );
    let wrong_native = admission(&declaration, "WorksetProposalAddition", &input);
    assert_eq!(
        propose_admitted_workset(
            &observed,
            &declaration,
            &wrong_native.admitted().unwrap(),
            Some(candidate(&checked))
        ),
        Err(E::NativeOwner)
    );
    assert_eq!(
        propose_admitted_workset(&observed, &declaration, &proof, None),
        Err(E::NativeSourceBasis)
    );
    let wrong_report = report("third");
    assert_eq!(
        propose_admitted_workset(
            &observed,
            &declaration,
            &proof,
            Some(candidate(&wrong_report))
        ),
        Err(E::NativeSourceBasis)
    );
    let wrong_body = body(&a, 2);
    assert_eq!(
        propose_admitted_workset(&wrong_body, &declaration, &proof, Some(candidate(&checked))),
        Err(E::WrongBody)
    );
    let later = observed.admit_plot(b, "sign/later".into()).unwrap();
    assert_eq!(
        propose_admitted_workset(&later, &declaration, &proof, Some(candidate(&checked))),
        Err(E::StaleWorkset)
    );
    assert_eq!(observed, before);
    let remove = program("propose-retain-one");
    let input = observation(&remove.input_type, &later, &a)
        .canonical_bytes()
        .unwrap();
    let bytes = remove.evaluate(&input).unwrap();
    let removal = admission(&declaration, "WorksetProposalInfo", &bytes);
    let normalized =
        propose_admitted_workset(&later, &declaration, &removal.admitted().unwrap(), None).unwrap();
    assert_eq!(normalized.proposed_workset, BodyWorkset::one(a).unwrap());
    assert_eq!(
        propose_admitted_workset(
            &later,
            &declaration,
            &removal.admitted().unwrap(),
            Some(candidate(&checked))
        ),
        Err(E::NativeSourceBasis)
    );
}
