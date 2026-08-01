use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use conduit_compile::{InstalledProfile, compile_source};
use conduit_learned::{
    lifecycle::{
        register_deterministic_lifecycle_provider, register_deterministic_training_provider,
    },
    register_deterministic_inference_provider, register_learned_contracts,
};
use conduit_runtime::{AvailabilityState, Registry};

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn exact_inference_binding_runs_through_the_production_executor() {
    let source = include_str!("../../../examples/learned-fixed-inference.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let inference = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "learned/infer")
        .unwrap();
    assert_eq!(
        inference.implementation.id,
        "conduit.learned/fixed-linear-rust"
    );
    assert_eq!(
        inference.artifact,
        "conduit.learned/fixed-linear-rust-artifact"
    );

    for (path, expected) in [
        (
            "examples/learned-fixed-inference.panel",
            "learned:i16:1x2:[35,-3]",
        ),
        (
            "examples/learned-inference-compose.panel",
            "LEARNED:I16:1X2:[35,-3]",
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
fn inference_contract_only_missing_and_stale_provider_states_are_distinct() {
    let source = include_str!("../../../examples/learned-fixed-inference.panel");
    let panel = conduit_panel::parse(source).unwrap();
    let mut contract_only = Registry::default();
    register_learned_contracts(&mut contract_only);
    assert_eq!(
        contract_only.node_availability("learned/infer").state,
        AvailabilityState::ContractOnly
    );
    assert_eq!(
        contract_only.resolve(&panel).unwrap_err().code,
        "CND-IMP-001"
    );

    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let mut stale = installed.input.clone();
    let candidate = stale
        .candidates
        .iter_mut()
        .find(|candidate| candidate.implementation.semantic_contract.id == "learned/infer")
        .unwrap();
    candidate.host_report.valid_until_tick = stale.current_tick - 1;
    stale.seal().unwrap();
    assert_eq!(
        compile_source(source, &stale).unwrap_err().code(),
        "CND-CMP-006"
    );
}

#[test]
fn inference_schema_runtime_device_resource_and_bounds_fail_closed() {
    for (from, to, code) in [
        (
            "conduit-fixed-linear",
            "unsupported-format",
            "CND-LEARN-002",
        ),
        (
            "conduit.learned/runtime/rust-fixed-linear",
            "conduit.learned/runtime/missing",
            "CND-LEARN-006",
        ),
        ("opset = 0", "opset = 1", "CND-LEARN-002"),
        (
            "conduit.learned/device/cpu-reference",
            "conduit.learned/device/missing",
            "CND-LEARN-006",
        ),
        (
            "conduit.learned/resource/cpu-reference-0",
            "conduit.learned/resource/cpu-reference-1",
            "CND-LEARN-006",
        ),
        (
            "input_shape = \"1x4\"",
            "input_shape = \"1x5\"",
            "CND-LEARN-006",
        ),
        ("maximum_batch = 1", "maximum_batch = 2", "CND-LEARN-006"),
        (
            "sensitivity = \"public\"",
            "sensitivity = \"secret\"",
            "CND-LEARN-003",
        ),
    ] {
        let source =
            include_str!("../../../examples/learned-fixed-inference.panel").replacen(from, to, 1);
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
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success(), "mutation {from} unexpectedly ran");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn learned_values_remain_domain_specific_and_training_free() {
    let mut registry = Registry::default();
    register_learned_contracts(&mut registry);
    let ids = registry
        .contracts()
        .map(|contract| contract.id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.contains(&"learned/model/literal"));
    assert!(ids.contains(&"learned/infer"));
    assert!(
        !ids.iter()
            .any(|id| id.contains("train") || id.contains("promotion") || id.contains("job"))
    );
    assert!(
        !ids.iter()
            .any(|id| id.starts_with("speech/") || id.starts_with("robotics/"))
    );
}

#[test]
fn exact_lifecycle_plan_pins_training_evaluation_and_promotion_authority() {
    let source = include_str!("../../../examples/learned-lifecycle.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    register_deterministic_lifecycle_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let promotion = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "learned/promote")
        .unwrap();
    assert_eq!(
        promotion.implementation.id,
        "conduit.learned/promote-deterministic"
    );
    assert_eq!(document.authorities.len(), 1);
    assert_eq!(
        document.authorities[0].effect.action,
        "conduit.action/promote"
    );

    for (path, expected) in [
        (
            "examples/learned-lifecycle-standalone.panel",
            "learned:dataset:tiny:train:4:public",
        ),
        (
            "examples/learned-evaluation.panel",
            "learned:evaluation:accuracy@1:4/4:not-approval",
        ),
        (
            "examples/learned-lifecycle.panel",
            "learned:promotion:learned/reference:acknowledged",
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
fn training_and_evaluation_are_runnable_without_a_promotion_provider_or_grant() {
    let source = include_str!("../../../examples/learned-evaluation.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    register_deterministic_training_provider(&mut registry).unwrap();
    assert_eq!(
        registry.node_availability("learned/promote").state,
        AvailabilityState::ContractOnly
    );
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    assert!(document.authorities.is_empty());
}

#[test]
fn learned_lifecycle_schema_resources_metrics_and_promotion_fail_closed() {
    for (from, to, code) in [
        (
            "sensitivity = \"public\"",
            "sensitivity = \"secret\"",
            "CND-LEARN-012",
        ),
        (
            "sha256:d5ac227d73ef18638d38b51c67b816148cd18c837680cc2fb827e4ef773c5145",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "CND-LEARN-011",
        ),
        ("maximum_work = 64", "maximum_work = 65", "CND-LEARN-013"),
        ("metric_version = 1", "metric_version = 2", "CND-LEARN-018"),
        (
            "target_slot = \"learned/reference\"",
            "target_slot = \"learned/unapproved\"",
            "CND-LEARN-019",
        ),
    ] {
        let source =
            include_str!("../../../examples/learned-lifecycle.panel").replacen(from, to, 1);
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
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success(), "mutation {from} unexpectedly ran");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
