use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use conduit_compile::{InstalledProfile, compile_source};
use conduit_runtime::{AvailabilityState, Registry};
use conduit_spatial::{
    register_deterministic_spatial_data_provider, register_deterministic_spatial_provider,
    register_spatial_contracts, register_spatial_data_contracts,
};

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn run_source(source: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn exact_spatial_bindings_run_through_the_production_executor() {
    let source = include_str!("../../../examples/spatial-transform-compose.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_spatial_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let compose = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "spatial/transform/compose")
        .unwrap();
    assert_eq!(
        compose.implementation.id,
        "conduit.spatial/compose-deterministic"
    );
    assert_eq!(compose.artifact, "conduit.spatial/compose-artifact");

    for (path, expected) in [
        (
            "examples/spatial-transform-compose.panel",
            "spatial:point:sensor:[1000,500,10000]:clock/fixture@10:uncertainty=0spatial:point:camera:[1109,719,10330]:clock/fixture@10:uncertainty=0",
        ),
        (
            "examples/spatial-transform-interpolate.panel",
            "spatial:point:camera:[1010,520,10030]:clock/fixture@11:uncertainty=0",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn spatial_contract_only_missing_and_stale_provider_states_are_distinct() {
    let source = include_str!("../../../examples/spatial-transform-interpolate.panel");
    let panel = conduit_panel::parse(source).unwrap();
    let mut contract_only = Registry::default();
    register_spatial_contracts(&mut contract_only);
    assert_eq!(
        contract_only
            .node_availability("spatial/transform/interpolate")
            .state,
        AvailabilityState::ContractOnly
    );
    assert_eq!(
        contract_only.resolve(&panel).unwrap_err().code,
        "CND-IMP-001"
    );

    let mut registry = Registry::hosted_primitives();
    register_deterministic_spatial_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let mut stale = installed.input.clone();
    stale
        .candidates
        .iter_mut()
        .find(|candidate| {
            candidate.implementation.semantic_contract.id == "spatial/transform/interpolate"
        })
        .unwrap()
        .host_report
        .valid_until_tick = stale.current_tick - 1;
    stale.seal().unwrap();
    assert_eq!(
        compile_source(source, &stale).unwrap_err().code(),
        "CND-CMP-006"
    );
}

#[test]
fn spatial_time_rotation_uncertainty_calibration_and_bounds_fail_closed() {
    let composition = include_str!("../../../examples/spatial-transform-compose.panel");
    let interpolation = include_str!("../../../examples/spatial-transform-interpolate.panel");
    for (source, from, to, code) in [
        (
            composition,
            "quarter_turns_z = 0",
            "quarter_turns_z = 4",
            "CND-SPATIAL-003",
        ),
        (
            composition,
            "valid_until_tick = 20",
            "valid_until_tick = 9",
            "CND-SPATIAL-004",
        ),
        (
            composition,
            "uncertainty_um = 0",
            "uncertainty_um = 11",
            "CND-SPATIAL-006",
        ),
        (
            composition,
            "clock = \"clock/fixture\" tick = 10",
            "clock = \"clock/other\" tick = 10",
            "CND-SPATIAL-005",
        ),
        (
            composition,
            "sha256:5151515151515151515151515151515151515151515151515151515151515151",
            "sha256:0051515151515151515151515151515151515151515151515151515151515151",
            "CND-SPATIAL-007",
        ),
        (
            interpolation,
            "maximum_history_values = 2",
            "maximum_history_values = 3",
            "CND-SPATIAL-008",
        ),
        (
            interpolation,
            "tick = 11 maximum_window_ticks = 4",
            "tick = 15 maximum_window_ticks = 4",
            "CND-SPATIAL-004",
        ),
    ] {
        let output = run_source(&source.replacen(from, to, 1));
        assert!(!output.status.success(), "mutation {from} unexpectedly ran");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn spatial_contracts_remain_domain_specific() {
    let mut registry = Registry::default();
    register_spatial_contracts(&mut registry);
    let ids = registry
        .contracts()
        .map(|contract| contract.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids.iter().filter(|id| id.starts_with("spatial/")).count(),
        10
    );
    assert!(
        !ids.iter()
            .any(|id| id.starts_with("robot/") || id.starts_with("speech/"))
    );
}

#[test]
fn exact_bounded_scan_transform_and_grid_run_through_the_production_executor() {
    let source = include_str!("../../../examples/spatial-scan-grid.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_spatial_provider(&mut registry).unwrap();
    register_deterministic_spatial_data_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    for (contract, implementation, artifact) in [
        (
            "spatial/scan/fixture",
            "conduit.spatial/scan-fixture",
            "conduit.spatial/scan-fixture-artifact",
        ),
        (
            "spatial/scan/transform",
            "conduit.spatial/scan-transform-reference",
            "conduit.spatial/scan-transform-reference-artifact",
        ),
        (
            "spatial/grid/from-scan",
            "conduit.spatial/grid-from-scan-reference",
            "conduit.spatial/grid-from-scan-reference-artifact",
        ),
    ] {
        let node = document
            .nodes
            .iter()
            .find(|node| node.contract.id == contract)
            .unwrap();
        assert_eq!(node.implementation.id, implementation);
        assert_eq!(node.artifact, artifact);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .arg(workspace_file("examples/spatial-scan-grid.panel"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "spatial:grid:map:2x2:occupied=2:coverage=complete"
    );
}

#[test]
fn transform_only_host_remains_conforming_without_scan_or_map_provider() {
    let mut registry = Registry::hosted_primitives();
    register_deterministic_spatial_provider(&mut registry).unwrap();
    register_spatial_data_contracts(&mut registry);
    assert_eq!(
        registry.node_availability("spatial/transform/apply").state,
        AvailabilityState::ProviderAvailable
    );
    assert_eq!(
        registry.node_availability("spatial/scan/fixture").state,
        AvailabilityState::ContractOnly
    );
    assert_eq!(
        registry.node_availability("spatial/grid/from-scan").state,
        AvailabilityState::ContractOnly
    );
}
