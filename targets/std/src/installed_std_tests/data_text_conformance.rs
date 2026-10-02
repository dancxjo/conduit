use super::{host, installed_std, RecordingTimer};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_kernel::KernelEventKind;
use conduit_plot::parse;
use std::collections::BTreeMap;

#[test]
fn text_save_then_load_uses_one_shared_pool_and_no_play_allocations() {
    let mut host = host("data-text-host");
    let plot = parse(
        "plot data_text_round_trip {\n source: conduit-test/data-text-source\n save: data/save/text\n load: data/load/text\n save-recovery: conduit-test/data-save-terminal-recovery\n load-recovery: conduit-test/data-load-terminal-recovery\n sink: conduit-test/data-text-sink\n source.value >> save.value\n save.data >> load.data\n save.data >> save-recovery.success\n load.value >> sink.value\n load.value >> load-recovery.success\n save.data! >> save-recovery.terminal\n load.value! >> load-recovery.terminal\n}\n",
        &installed_std::test_catalog(),
    )
    .expect("data Text round trip parses");
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_placements(&plot, &hosts).expect("data Text placements resolve");
    let plan = conduit_planner::plan_with_options(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("data Text round trip plans through exact std offers");
    let fragment = &plan.fragments[0];
    let save = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_data::DATA_SAVE_TEXT_KIND)
        .expect("save placement exists");
    let load = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_data::DATA_LOAD_TEXT_KIND)
        .expect("load placement exists");
    assert_eq!(save.resources.len(), 1);
    assert_eq!(load.resources.len(), 1);
    assert_eq!(save.resources[0].pool_id, load.resources[0].pool_id);
    assert_eq!(
        save.resources[0].class_id.as_str(),
        conduit_std_offers::DATA_TEXT_GENERATION_RESOURCE_CLASS
    );
    assert!(save.authority.is_empty());
    assert!(load.authority.is_empty());
    assert!(fragment.placements.iter().all(|placement| {
        !matches!(
            placement.kind_id.as_str(),
            "state/keep" | "state/keep/optional" | "state/value"
        )
    }));

    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("data Text round trip executes through the production kernel");
    assert!(String::from_utf8(output).is_ok());
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn fifth_publication_is_one_exact_recovered_typed_terminal() {
    let mut source = String::from("plot data_text_pressure {\n");
    for index in 0..5 {
        source.push_str(&format!(
            " source-{index}: conduit-test/data-text-source\n save-{index}: data/save/text\n recovery-{index}: conduit-test/data-save-terminal-recovery\n source-{index}.value >> save-{index}.value\n save-{index}.data >> recovery-{index}.success\n save-{index}.data! >> recovery-{index}.terminal\n"
        ));
    }
    source.push_str("}\n");
    let plot = parse(&source, &installed_std::test_catalog()).expect("pressure plot parses");
    let mut host = host("data-text-pressure-host");
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&plot, &hosts)
        .expect("data Text pressure placements resolve");
    let plan = conduit_planner::plan_with_options(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("five finite publications plan against one admitted pool");
    let save_pools = plan.fragments[0]
        .placements
        .iter()
        .filter(|placement| placement.kind_id.as_str() == conduit_data::DATA_SAVE_TEXT_KIND)
        .map(|placement| placement.resources[0].pool_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(save_pools.len(), 5);
    assert!(save_pools.iter().all(|pool| pool == &save_pools[0]));

    let mut output = Vec::with_capacity(2_048);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("generation pressure is recovered as typed terminal truth");
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == KernelEventKind::SemanticAbnormal)
            .count(),
        1
    );
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == KernelEventKind::SemanticAbnormalRecovered)
            .count(),
        1
    );
}
