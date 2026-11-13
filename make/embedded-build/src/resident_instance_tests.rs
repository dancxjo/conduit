//! Same checked program, distinct initialized custody and exact Plan selection.
use crate::resident_expression_tests::compiled_rust;
use crate::*;
use conduit_core::PortTemporal;
fn bounds() -> ResidentExpressionBounds {
    ResidentExpressionBounds {
        native: conduit_plot::rust_binding::PreparedNativeBounds {
            canonical_bytes: 4096,
            scopes: 64,
            invariants: 64,
            contracts: 64,
        },
        maximum_occurrences: 4096,
        maximum_program_bytes: 512 * 1024,
        maximum_generated_bytes: 1024 * 1024,
        private_work_units: 1000,
    }
}
#[test]
fn repeated_code_has_distinct_owners_and_cannot_bind_its_sibling_selection() {
    let (document, program) = crate::resident_expression_tests::context();
    let prepare = |label| {
        PreparedResidentExpression::prepare_instance(
            &document,
            &program,
            PortTemporal::Value,
            bounds(),
            Some(label),
        )
        .unwrap()
    };
    let left = prepare("checked-source/left");
    let right = prepare("checked-source/right");
    assert_eq!(left.kind_id(), right.kind_id());
    assert_ne!(left.artifact_id(), right.artifact_id());
    assert_ne!(
        left.offer_description().capability_id,
        right.offer_description().capability_id
    );
    let plan = crate::resident_expression_tests::plan(&document, &program, &right, 1000);
    let fragment = &plan.fragments[0];
    let placement = &fragment.placements[1].placement_id;
    assert!(right.bind(&plan, &fragment.fragment_id, placement).is_ok());
    assert!(matches!(
        left.bind(&plan, &fragment.fragment_id, placement),
        Err(ResidentExpressionRefusal::Selection)
    ));
    assert!(matches!(
        generate_resident_library(&[prepare("same"), prepare("same")], 4 * 1024 * 1024),
        Err(ResidentExpressionRefusal::Selection)
    ));
    let code = generate_resident_library(&[left, right], 4 * 1024 * 1024).unwrap();
    compiled_rust::verify_with_libraries(&code,
        "fn main(){let owner=Initialized::prepare().unwrap();let left=owner.available_identity(0).unwrap();let right=owner.available_identity(1).unwrap();assert_eq!(left.kind,right.kind);assert_eq!(left.computation.text_digest,right.computation.text_digest);assert_ne!(left.artifact,right.artifact);assert_eq!(owner.offers().into_iter().flatten().count(),2);let backs=owner.into_backs();assert!(backs.0.is_some()&&backs.1.is_some());}",
        "distinct-resident-instances",&["conduit_kernel","conduit_plot","conduit_core","conduit_semantic_catalog","conduit_plan_lowering"]);
}
#[test]
fn invalid_instance_labels_refuse_before_code_preparation() {
    let (document, program) = crate::resident_expression_tests::context();
    for label in [String::new(), "x".repeat(1025)] {
        assert!(matches!(
            PreparedResidentExpression::prepare_instance(
                &document,
                &program,
                PortTemporal::Value,
                bounds(),
                Some(&label)
            ),
            Err(ResidentExpressionRefusal::Bounds)
        ));
    }
}

#[test]
fn cold_codebook_refusal_reports_required_bytes_without_enlarging_the_limit() {
    let (document, program) = crate::resident_expression_tests::context();
    let mut selected = bounds();
    selected.maximum_program_bytes = 0;
    assert!(
        matches!(PreparedResidentExpression::prepare(&document,&program,PortTemporal::Value,selected),Err(ResidentExpressionRefusal::ProgramBytes{required,maximum:0}) if required > 0)
    );
    selected = bounds();
    selected.maximum_generated_bytes = 1;
    assert!(
        matches!(PreparedResidentExpression::prepare(&document,&program,PortTemporal::Value,selected),Err(ResidentExpressionRefusal::GeneratedBytes{required,maximum:1}) if required > 1)
    );
}
