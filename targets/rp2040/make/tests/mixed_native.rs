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
    qualify(library, 1024);
}
#[test]
fn actual_five_entry_library_admits_only_the_selected_program_storage() {
    use conduit_host_make::*;
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let configuration = parse_host_configuration_conduit(include_str!(
        "../../profiles/pico-w-house-green.host.conduit"
    ))
    .unwrap();
    let checked = check_host_configuration(
        configuration,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap();
    qualify(
        conduit_host_rp2040::checked_ht16k33_resident_library(&checked).unwrap(),
        1024,
    );
}
#[test]
fn disabled_native_issuance_refuses_an_otherwise_approved_workset() {
    let (_, _, image) = support::context();
    let library = conduit_embedded_build::generate_resident_library(&[image], 1024 * 1024).unwrap();
    qualify(library, 0);
}
#[test]
fn absent_execution_policy_cannot_authorize_native_grants_from_ready_equipment() {
    let (_, _, image) = support::context();
    let library = conduit_embedded_build::generate_resident_library(&[image], 1024 * 1024).unwrap();
    qualify_policy(library, 1024, false);
}
fn qualify(library: String, operation_ceiling: u32) {
    qualify_policy(library, operation_ceiling, true);
}
fn qualify_policy(library: String, operation_ceiling: u32, delegated: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let resident = include_str!("../../firmware/host/src/resident.rs").replace(
        "include!(concat!(env!(\"OUT_DIR\"), \"/resident.rs\"));",
        &library,
    );
    let generated = format!(
        r#"
extern crate alloc;
extern crate self as embassy_time;
#[allow(dead_code)]#[path={platform:?}]pub mod supervisor_platform;
pub use supervisor_platform::{{Instant,Duration,Timer}};
mod bootsel{{pub const CONTROL_FRAME_BYTES:usize=128;}}
mod transport{{pub const LIMIT:usize=4096;pub type Link=crate::supervisor_platform::Link;}}
#[allow(dead_code)]#[path={supervisor:?}]mod execution_supervisor;
#[allow(dead_code)]#[path={selection:?}]mod execution_selection;
#[allow(dead_code)]#[path={mixed_supervisor:?}]mod mixed_supervision;
#[allow(dead_code)]#[path={release:?}]mod retired_resources;
mod resident{{{resident}}}
#[allow(dead_code)]#[path={resident_preparation:?}]mod resident_preparation;
#[allow(dead_code)]#[path={resident_supervisor:?}]mod resident_supervision;
mod config{{pub const CONFIGURATION_ID:&str="fixture/config";pub const IMAGE_ID:&str="fixture/image";
pub const PREPARATION_DELEGATION:Option<conduit_body::HostPreparationDelegation>=Some(conduit_body::HostPreparationDelegation{{authorizer:conduit_body::HostPreparationAuthorizer::SeedOwner,maximum_assignment_bytes:4096,maximum_runtime_state_bytes:16384}});
pub const EXECUTION_DELEGATION:Option<conduit_body::HostExecutionDelegation>=if {delegated} {{Some(conduit_body::HostExecutionDelegation{{authorizer:conduit_body::HostExecutionAuthorizer::SeedOwner,maximum_active_worksets:1,continuation:None}})}} else {{None}};
pub const I2C_CONTROLLER:u8=1;pub const I2C_CALL_ADDRESS_RANGE:Option<(u8,u8)>=Some((112,119));pub const I2C_CALL_MAXIMUM_OPERATIONS:u32={operation_ceiling};}}
#[allow(dead_code)]#[path={issuance:?}]mod i2c_issuance;
#[allow(dead_code)]#[path={native:?}]mod native_preparation;
#[allow(dead_code)]#[path={mixed:?}]mod mixed_preparation;
#[allow(dead_code)]#[path={workset:?}]mod mixed_workset;
"#,
        resident_preparation = root.join("../firmware/host/src/resident_preparation.rs"),
        resident_supervisor = root.join("../firmware/host/src/resident_supervision.rs"),
        platform = root.join("tests/mixed_native/supervisor_platform.rs"),
        selection = root.join("../firmware/host/src/execution_selection.rs"),
        supervisor = root.join("../firmware/host/src/execution_supervisor.rs"),
        mixed_supervisor = root.join("../firmware/host/src/mixed_supervision.rs"),
        release = root.join("../firmware/host/src/retired_resources.rs"),
        issuance = root.join("../firmware/host/src/i2c_issuance.rs"),
        native = root.join("../firmware/host/src/native_preparation.rs"),
        mixed = root.join("../firmware/host/src/mixed_preparation.rs"),
        workset = root.join("../firmware/host/src/mixed_workset.rs")
    );
    let exercise = format!(
        r#"
#[path={support:?}]mod support;
#[path={probe:?}]mod probe;
#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;
#[path={exercise:?}]mod exercise;
#[path={prepared_values:?}]mod prepared_values;
fn main(){{prepared_values::run();exercise::exercise();}}
"#,
        prepared_values = root.join("tests/mixed_native/prepared_values.rs"),
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
            "conduit_mcu_host_runtime",
            "conduit_plot",
            "conduit_body",
            "conduit_embedded_build",
            "conduit_kernel",
            "conduit_plan_lowering",
            "conduit_semantic_catalog",
            "conduit_i2c_transaction",
            "ed25519_dalek",
            "serde_json",
            "sha2",
            "serde",
            "serde_json_core",
            "heapless",
            "critical_section",
            "static_cell",
        ],
    );
}
