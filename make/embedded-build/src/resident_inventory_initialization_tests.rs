//! Caller-owned inventory preparation publishes nothing after partial failure.
use crate::resident_expression_tests::compiled_rust;
use crate::*;

fn library() -> String {
    let (document, program) = crate::resident_expression_tests::context();
    let bounds = ResidentExpressionBounds {
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
    };
    let images = ["first", "second"].map(|label| {
        PreparedResidentExpression::prepare_instance(
            &document,
            &program,
            conduit_core::PortTemporal::Value,
            bounds,
            Some(label),
        )
        .unwrap()
    });
    generate_resident_library(&images, 4 * 1024 * 1024).unwrap()
}
fn verify(code: &str, exercise: &str, name: &str) {
    compiled_rust::verify_with_libraries(
        code,
        exercise,
        name,
        &[
            "conduit_kernel",
            "conduit_plot",
            "conduit_core",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
        ],
    );
}

#[test]
fn in_place_preparation_preserves_existing_owners_on_repeat() {
    verify(
        &library(),
        r#"
fn main(){
 let mut owner=Initialized::empty();
 assert!(owner.offers().iter().all(Option::is_none));
 owner.prepare_into().unwrap();
 let offers=owner.offers();
 assert_eq!(offers.iter().flatten().count(),2);
 assert!(matches!(owner.prepare_into(),Err(PreparationRefusal::Occupied(0))));
 assert_eq!(owner.offers(),offers);
 assert!(owner.available_identity(0).is_some());
 assert!(owner.available_identity(1).is_some());
}
"#,
        "in-place-resident-inventory",
    );
}

#[test]
fn failed_later_entry_leaves_no_partial_inventory_and_can_be_retried() {
    let code = library();
    assert_eq!(code.matches("entry_1::Initialized::prepare()").count(), 1);
    let code = code.replace("entry_1::Initialized::prepare()", "forced_refusal()");
    let code = format!("{code}\nfn forced_refusal()->Result<entry_1::Initialized,entry_1::PreparationRefusal>{{Err(entry_1::PreparationRefusal::Back(conduit_semantic_catalog::StaticExpressionBackRefusal::Work))}}\n");
    verify(
        &code,
        r#"
fn main(){
 let mut owner=Initialized::empty();
 for _ in 0..2 {
  assert!(matches!(owner.prepare_into(),Err(PreparationRefusal::Entry1(_))));
  assert!(owner.offers().iter().all(Option::is_none));
  assert!(owner.available_identity(0).is_none());
  assert!(owner.available_identity(1).is_none());
 }
}
"#,
        "in-place-resident-inventory-refusal",
    );
}
