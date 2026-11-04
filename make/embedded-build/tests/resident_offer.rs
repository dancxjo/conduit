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
