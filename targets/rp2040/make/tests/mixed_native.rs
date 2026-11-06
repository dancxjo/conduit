//! Actual Root mixed-kernel composition; inert CPU owners, never device admission.
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/compiled_rust.rs"]
mod compiled_rust;
#[path = "mixed_native/support.rs"]
mod support;
#[test]
fn same_kernel_retains_both_preparations_and_runs_the_selected_native_node() {
    let (_, _, image) = support::context();
    let library = conduit_embedded_build::generate_resident_library(&[image], 1024 * 1024).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated = format!(
        r#"
extern crate alloc;
mod resident{{pub mod generated{{{library}}}}}
#[allow(dead_code)]#[path={native:?}]mod native_preparation;
#[allow(dead_code)]#[path={mixed:?}]mod mixed_preparation;
"#,
        native = root.join("../firmware/host/src/native_preparation.rs"),
        mixed = root.join("../firmware/host/src/mixed_preparation.rs")
    );
    let exercise = format!(
        r#"
#[path={support:?}]mod support;
#[path={probe:?}]mod probe;
#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;
#[path={exercise:?}]mod exercise;
fn main(){{exercise::exercise();}}
"#,
        exercise = root.join("tests/mixed_native/exercise.rs"),
        support = root.join("tests/mixed_native/support.rs"),
        probe = root.join("../../../architecture/plot/tests/common/allocation_probe.rs")
    );
    compiled_rust::verify_with_libraries(
        &generated,
        &exercise,
        "root-mixed-native",
        &[
            "conduit_core",
            "conduit_plot",
            "conduit_body",
            "conduit_embedded_build",
            "conduit_kernel",
            "conduit_plan_lowering",
            "conduit_semantic_catalog",
            "conduit_i2c_transaction",
        ],
    );
}
