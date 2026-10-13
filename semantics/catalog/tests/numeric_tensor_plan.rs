#![cfg(all(feature = "kernel-step", feature = "plot-catalog"))]
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_semantic_catalog::*;
fn source(accuracy: &str) -> String {
    let source = format!(
        r#"plot main {{
        projection: tensor/evaluate(values = [0.1, 0.1 + 0.2], dimensions = [2],
            axes = [{{ role: feature(empty), identity: none(empty), unit: none(empty) }}],
            requirement = {{ semantics: mathematical(empty), representation: binary64(empty),
                rounding: nearestEven(empty), accuracy: {accuracy},
                budget: {{ storageBytes: 1048576, workSteps: 100000, limbs: 128, symbolicNodes: 128 }},
                parameters: [], reproducible: true,
                exceptions: {{ invalid:false, divisionByZero:false, overflow:false, underflow:false, inexact:true }} }})
    }}."#
    );
    let start = source.find("projection:").unwrap();
    let end = source.rfind('}').unwrap();
    format!(
        "plot main {{\n{}\n}}.",
        source[start..end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    )
}
fn expanded(source: &str) -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_mathematical_kinds(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    expand_canonical_plot(&checked, "main", &profile).unwrap()
}
fn plan(expanded: &conduit_plot::ExpandedCanonicalPlot) -> Result<Plan, String> {
    let kind = numeric_tensor_kind(&expanded.gears[0]).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/numeric-tensor"),
        boot_id: BootId::from("boot/numeric-tensor"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/numeric-tensor@1"),
        bases: vec![],
        planner_capabilities: vec![],
        resources: vec![resource_offer(
            "memory/numeric-tensor",
            RUNTIME_MEMORY_RESOURCE_CLASS,
            8_388_608,
        )],
        capabilities: vec![numeric_tensor_offer(
            kind,
            "conduit.std/numeric-tensor@1",
            "conduit.std/prepared-numeric-tensor@1",
        )],
    };
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(expanded, &hosts)
        .map_err(|error| format!("{error:?}"))?;
    conduit_planner::plan_expanded_canonical(expanded, &hosts, &placements, &[])
        .map_err(|error| format!("{error:?}"))
}
#[test]
fn authored_ordered_coordinates_execute_a_selected_tensor_back_with_consumer_loss_custody() {
    assert!(plan(&expanded(&source("exact(empty)"))).is_err());
    let expanded = expanded(&source("absolute(0.0000001)"));
    let mut plan = plan(&expanded).unwrap();
    let placement = &mut plan.fragments[0].placements[0];
    assert!(placement
        .realization_properties
        .iter()
        .any(|property| property.profile() == &numeric_tensor_witness_profile()));
    let mut back = prepare_numeric_tensor(placement).unwrap();
    assert_eq!(back.consumer_summary().elements, 2);
    assert_eq!(back.consumer_summary().bytes, 16);
    let summaries = back.projection().coordinate_projection_summaries().unwrap();
    assert!(summaries.iter().all(|summary| summary.losses
        [conduit_core::projection::ProjectionLoss::Precision as usize]
        == 1));
    let encoded = <NumericTensorBack as StepBack<1>>::prepared_output(&back, PortId(0))
        .unwrap()
        .to_vec();
    let tensor = conduit_data::TensorValue::decode(&encoded).unwrap();
    back.projection().readmit(&tensor).unwrap();
    let mut blocked = StepIo::test_frame([None], [false], [None], None, 4);
    assert_eq!(
        back.step(&mut blocked, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Await
    );
    let mut ready = StepIo::test_frame([None], [true], [Some(4096)], None, 4);
    assert_eq!(
        back.step(&mut ready, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        back.step(&mut ready, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    for binding in &mut placement.resources {
        binding.units = 0;
    }
    assert!(prepare_numeric_tensor(placement).is_err());
}
