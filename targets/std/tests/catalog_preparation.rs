//! Measures the default Std package graph; do not pass AI cache features manually.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_ai::fixed_numeric_catalog::{
    fixed_numeric_contracts, fixed_numeric_type, fixed_numeric_types,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostConfig};
use std::time::Instant;

#[test]
#[ignore = "manual fresh-process preparation allocation and latency measurement"]
fn default_std_graph_retains_exact_catalogues_across_preparation() {
    let prepare = || {
        let started = Instant::now();
        let (catalogues, allocation) = allocation_probe::observe(|| {
            (
                fixed_numeric_types().unwrap(),
                fixed_numeric_contracts().unwrap(),
            )
        });
        (catalogues, allocation, started.elapsed().as_nanos())
    };
    let (cold, cold_alloc, cold_ns) = prepare();
    let refusal = fixed_numeric_type("NoSuchNumericType").unwrap_err();
    let (warm, warm_alloc, warm_ns) = prepare();
    assert_eq!(cold, warm);
    assert_eq!(
        refusal,
        fixed_numeric_type("NoSuchNumericType").unwrap_err()
    );
    let host = || {
        StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("catalogue-measurement-host"),
            boot_id: BootId::from("catalogue-measurement-boot"),
            offer_generation: OfferGeneration(1),
        })
    };
    let mut first_host = host();
    let mut second_host = host();
    assert_eq!(first_host.advertisement(), second_host.advertisement());
    // Ordinary product compatibility is distinct from the catalogue-only timing.
    let first = ordinary_greet(&mut first_host);
    let second = ordinary_greet(&mut second_host);
    assert_eq!(first, second);
    println!(
        "STD_CATALOG_COMPATIBILITY={}",
        serde_json::json!({
            "workload": "ordinary Std Greet welcome; no numerical Play claim",
            "plan_id": first.0, "output": first.1, "observations": first.2,
        })
    );
    println!("STD_CATALOG_PREPARATION={{\"scope\":\"default Std dependency graph numeric catalogue preparation; excludes Play and Host construction\",\"cold_allocations\":{},\"warm_allocations\":{},\"cold_reallocations\":{},\"warm_reallocations\":{},\"cold_peak_new_bytes\":{},\"warm_peak_new_bytes\":{},\"cold_ns\":{},\"warm_ns\":{},\"types\":{},\"contracts\":{}}}",
        cold_alloc.allocations, warm_alloc.allocations, cold_alloc.reallocations,
        warm_alloc.reallocations, cold_alloc.peak_bytes, warm_alloc.peak_bytes,
        cold_ns, warm_ns, cold.0.len(), cold.1.len());
}

fn ordinary_greet(host: &mut StdHost) -> (String, String, usize) {
    use conduit_core::{ObservationKind, TerminalDisposition};
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
        StartupCatalog,
    };
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../../plots/greet/main.conduit")),
        &startup,
    )
    .unwrap();
    let expanded = expand_canonical_plot(&checked, "welcome", &profiles).unwrap();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let mut output = Vec::with_capacity(4096);
    let report = host
        .run_fragment_to(
            plan.fragments[0].clone(),
            &mut output,
            &mut conduit_std_host::ThreadTimer,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let signs = report.observations.len();
    let kernel = report
        .kernel
        .expect("ordinary installed kernel must execute");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("WelcomeTravis\n"));
    (plan.plan_id.as_str().into(), output, signs)
}
