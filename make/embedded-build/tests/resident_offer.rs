//! Emitted initialized code feeds the ordinary planner while its owner is alive.
#[path = "../../../architecture/plot/tests/common/compiled_rust.rs"]
mod compiled_rust;
#[allow(dead_code)] // The emitted executable runs the planner exercise.
#[path = "resident_offer/support.rs"]
mod support;
#[test]
fn live_resident_owner_supplies_exact_offer_for_checked_source_planning() {
    let (doc, program, temporal) = support::context();
    let image = conduit_embedded_build::PreparedResidentExpression::prepare(
        &doc,
        &program,
        temporal,
        support::bounds(1000),
    )
    .unwrap();
    let weak = conduit_embedded_build::PreparedResidentExpression::prepare(
        &doc,
        &program,
        temporal,
        support::bounds(1),
    )
    .unwrap();
    let code = format!(
        "mod image{{\n{}\n}}\nmod weak{{\n{}\n}}",
        image.source(),
        weak.source()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let probe = root.join("../../architecture/plot/tests/common/allocation_probe.rs");
    let helper = root.join("tests/resident_offer/support.rs");
    let exercise = format!(
        r#"
#[path={probe:?}]mod probe;
#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;
#[path={helper:?}]mod support;
fn main(){{
    let(owner,cost)=probe::observe(||image::Initialized::prepare().unwrap());
    assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
    let((),cost)=probe::observe(||{{assert!(!owner.offer_json().is_empty());
        assert!(matches!(weak::Initialized::prepare(),Err(weak::PreparationRefusal::Back(conduit_semantic_catalog::StaticExpressionBackRefusal::Work))));}});
    assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
    let offer:conduit_core::CapabilityOffer=serde_json::from_slice(owner.offer_json()).unwrap();
    let granted=support::exercise(offer);
    let back=owner.into_back();
    assert!(back.required_step_fuel()<=granted);
}}
"#,
        probe = probe.to_str().unwrap(),
        helper = helper.to_str().unwrap()
    );
    compiled_rust::verify_with_libraries(
        &code,
        &exercise,
        "initialized-resident-offer",
        &[
            "conduit_embedded_build",
            "conduit_plot",
            "conduit_core",
            "conduit_kernel",
            "conduit_semantic_catalog",
            "conduit_planner",
            "serde_json",
        ],
    );
}

#[test]
fn library_refuses_duplicate_inventory_bounds_and_partial_initialization() {
    use conduit_embedded_build::{generate_resident_library, PreparedResidentExpression as Image};
    let (doc, program, temporal) = support::context();
    let good = Image::prepare(&doc, &program, temporal, support::bounds(1000)).unwrap();
    let duplicate = Image::prepare(&doc, &program, temporal, support::bounds(1000)).unwrap();
    assert!(generate_resident_library(&[], 1024).is_err());
    assert!(generate_resident_library(&[good, duplicate], 2 * 1024 * 1024).is_err());
    let good = Image::prepare(&doc, &program, temporal, support::bounds(1000)).unwrap();
    assert!(generate_resident_library(std::slice::from_ref(&good), 1).is_err());
    // A distinct temporal Kind fails after the first actual owner initializes.
    let weak = Image::prepare(
        &doc,
        &program,
        conduit_core::PortTemporal::Value,
        support::bounds(1),
    )
    .unwrap();
    let library = generate_resident_library(&[good, weak], 2 * 1024 * 1024).unwrap();
    let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../architecture/plot/tests/common/allocation_probe.rs");
    let exercise = format!(
        r#"
#[path={probe:?}]mod probe;
#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;
fn main(){{
 let((),cost)=probe::observe(||{{
  match library::Initialized::prepare(){{
   Err(error@library::PreparationRefusal::Entry1(_))=>assert_eq!(error.entry(),1),
   _=>panic!("partial inventory must not become initialized"),
  }}
 }});
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
}}
"#,
        probe = probe.to_str().unwrap()
    );
    compiled_rust::verify_with_libraries(
        &format!("mod library{{\n{library}\n}}"),
        &exercise,
        "resident-library-partial-refusal",
        &["conduit_core", "conduit_plot", "conduit_semantic_catalog"],
    );
}
