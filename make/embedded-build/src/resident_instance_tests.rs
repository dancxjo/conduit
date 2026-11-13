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

#[cfg(feature = "execution-workspace")]
#[test]
fn shared_workspace_fabrication_seals_storage_and_preserves_exclusive_loans() {
    let (document, program) = crate::resident_expression_tests::context();
    let prepare = |storage| {
        PreparedResidentExpression::prepare_instance_with_storage(
            &document,
            &program,
            PortTemporal::Value,
            bounds(),
            Some("display/request"),
            storage,
        )
        .unwrap()
    };
    let owned = prepare(ResidentExecutionStorage::Owned);
    let shared = prepare(ResidentExecutionStorage::SharedWorkspace);
    assert_eq!(owned.kind_id(), shared.kind_id());
    assert_ne!(owned.artifact_id(), shared.artifact_id());
    let plan = crate::resident_expression_tests::plan(&document, &program, &shared, 1000);
    let f = &plan.fragments[0];
    assert!(shared
        .bind(&plan, &f.fragment_id, &f.placements[1].placement_id)
        .is_ok());
    assert!(matches!(
        owned.bind(&plan, &f.fragment_id, &f.placements[1].placement_id),
        Err(ResidentExpressionRefusal::Selection)
    ));
    // Mixed physical profiles must forward each exact owner's requirements;
    // the owned sibling still needs no scheduler workspace.
    let code = generate_resident_library(&[owned, shared], 4 * 1024 * 1024).unwrap();
    compiled_rust::verify_with_libraries(
        &code,
        r#"
fn main(){
 use conduit_kernel::scheduler::StepBack;
 let mut owner=Initialized::prepare().unwrap();
 assert!(core::mem::size_of::<entry_1::Back>() < core::mem::size_of::<entry_0::Back>());
 let loans=PreparedResidentLoans{owner:&mut owner,nodes:[Some(0),Some(1)]};
 assert_eq!(loans.execution_workspace_requirements_at::<1>(0),Some(None));
 let expected=loans.execution_workspace_requirements_at::<1>(1).unwrap().unwrap();
 assert!(loans.execution_workspace_requirements_at::<1>(2).is_none());
 let entries=loans.split();
 assert_eq!(entries[0].as_ref().unwrap().execution_workspace_requirements::<1>(),None);
 assert_eq!(entries[1].as_ref().unwrap().execution_workspace_requirements::<1>(),Some(expected));
 drop(entries);
 assert!(owner.available_identity(0).is_some());assert!(owner.available_identity(1).is_some());
 let borrowed=PreparedResidentLoans{owner:&mut owner,nodes:[Some(0),Some(1)]}.bind();
 assert!(StepBack::<1>::workspace_requirements(&borrowed[0].as_ref().unwrap().1).is_none());
 assert_eq!(StepBack::<1>::workspace_requirements(&borrowed[1].as_ref().unwrap().1),Some(expected));
 assert!(expected.output_bytes>0 && expected.scratch_bytes>0);
 drop(borrowed);
 assert!(owner.available_identity(0).is_none());
 assert!(owner.available_identity(1).is_none());
}
"#,
        "shared-workspace-resident-fabrication",
        &[
            "conduit_kernel",
            "conduit_plot",
            "conduit_core",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
        ],
    );
}

#[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
#[test]
fn schema_workspace_fabrication_seals_and_forwards_exact_profiles() {
    let (document, program) = crate::resident_expression_tests::context();
    let prepare = |storage| {
        PreparedResidentExpression::prepare_instance_with_storage(
            &document,
            &program,
            PortTemporal::Value,
            bounds(),
            Some("display/request"),
            storage,
        )
        .unwrap()
    };
    let canonical = prepare(ResidentExecutionStorage::SharedWorkspace);
    let schema = prepare(ResidentExecutionStorage::SharedWorkspaceSchema);
    assert_eq!(canonical.kind_id(), schema.kind_id());
    assert_ne!(canonical.artifact_id(), schema.artifact_id());
    let plan = crate::resident_expression_tests::plan(&document, &program, &schema, 1000);
    let f = &plan.fragments[0];
    assert!(schema
        .bind(&plan, &f.fragment_id, &f.placements[1].placement_id)
        .is_ok());
    assert!(matches!(
        canonical.bind(&plan, &f.fragment_id, &f.placements[1].placement_id),
        Err(ResidentExpressionRefusal::Selection)
    ));
    let code = generate_resident_library(&[canonical, schema], 4 * 1024 * 1024).unwrap();
    compiled_rust::verify_with_libraries(
        &code,
        r#"
fn main(){
 use conduit_kernel::{PortId, scheduler::StepBack};
 let mut owner=Initialized::prepare().unwrap();
 let loans=PreparedResidentLoans{owner:&mut owner,nodes:[Some(0),Some(1)]};
 assert_eq!(loans.execution_workspace_requirements_at::<1>(0),loans.execution_workspace_requirements_at::<1>(1));
 let backs=loans.bind();
 let canonical=&backs[0].as_ref().unwrap().1;
 let schema=&backs[1].as_ref().unwrap().1;
 assert!(StepBack::<1>::canonical_input_schema(canonical,PortId(0)).is_none());
 assert!(StepBack::<1>::canonical_output_schema(canonical,PortId(0)).is_none());
 assert!(StepBack::<1>::canonical_input_schema(schema,PortId(0)).is_some());
 assert!(StepBack::<1>::canonical_output_schema(schema,PortId(0)).is_some());
 assert!(StepBack::<1>::canonical_input_schema(schema,PortId(1)).is_none());
 assert!(StepBack::<1>::workspace_requirements(schema).is_some());
}
"#,
        "schema-workspace-resident-fabrication",
        &[
            "conduit_kernel",
            "conduit_plot",
            "conduit_core",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
        ],
    );
}
