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
    let (value_doc, value_program, value_temporal) =
        support::context_for(conduit_core::PortTemporal::Value);
    let sibling = conduit_embedded_build::PreparedResidentExpression::prepare(
        &value_doc,
        &value_program,
        value_temporal,
        support::bounds(1000),
    )
    .unwrap();
    let image_source = image.source().to_owned();
    let library =
        conduit_embedded_build::generate_resident_library(&[image, sibling], 2 * 1024 * 1024)
            .unwrap();
    let code = format!(
        "mod image{{\n{}\n}}\nmod weak{{\n{}\n}}\nmod library{{\n{library}\n}}",
        image_source,
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
#[path={kernel:?}]mod support_kernel;
#[path={refusals:?}]mod refusals;
#[path={library_helper:?}]mod library_checks;
#[path={reuse:?}]mod reuse;
#[path={combined:?}]mod combined;
#[path={body_scope:?}]mod body_scope;
fn main(){{
    let(owner,cost)=probe::observe(||image::Initialized::prepare().unwrap());
    assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
    let((),cost)=probe::observe(||{{assert!(!owner.offer_image().is_empty());
        assert!(matches!(weak::Initialized::prepare(),Err(weak::PreparationRefusal::Back(conduit_semantic_catalog::StaticExpressionBackRefusal::Work))));}});
    assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
    let offer=library_checks::decode(owner.offer_image());
    let assignment=support::exercise(offer);
    let active=assignment.active();
    assert_eq!(owner.computation_requirement(0),assignment.pins[0]);
    let mut owner=Some(owner);
    refusals::run(&mut owner,&assignment);
    let((node,back),cost)=probe::observe(||image::Initialized::bind_assigned_retained(&mut owner,assignment.selected(&active)).unwrap());
    assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
    assert_eq!(node,0);assert!(owner.is_none());
    assert!(matches!(image::Initialized::bind_assigned_retained(&mut owner,assignment.selected(&active)),Err(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)));
    assert!(back.required_step_fuel()<=assignment.tables.nodes[0].maximum_step_fuel);
    body_scope::run();
    combined::run(&assignment);
    reuse::run(&assignment);
    let library_back=library_checks::run(&assignment);
    support_kernel::run(library_back,assignment);

}}
"#,
        body_scope = root
            .join("tests/resident_offer/body_scope.rs")
            .to_str()
            .unwrap(),
        combined = root
            .join("tests/resident_offer/combined.rs")
            .to_str()
            .unwrap(),
        reuse = root.join("tests/resident_offer/reuse.rs").to_str().unwrap(),
        probe = probe.to_str().unwrap(),
        helper = helper.to_str().unwrap(),
        kernel = root
            .join("tests/resident_offer/kernel.rs")
            .to_str()
            .unwrap(),
        refusals = root
            .join("tests/resident_offer/refusals.rs")
            .to_str()
            .unwrap(),
        library_helper = root
            .join("tests/resident_offer/library.rs")
            .to_str()
            .unwrap()
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
            "conduit_plan_lowering",
            "conduit_planner",
            "conduit_body",
            "conduit_wire",
            "ed25519_dalek",
            "sha2",
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
        &[
            "conduit_kernel",
            "conduit_core",
            "conduit_plot",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
        ],
    );
}
