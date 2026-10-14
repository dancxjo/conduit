//! Oracle for the real reviewed physical-values Source and standing Plot entrance.
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    quantity_conversion::*, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
    TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = include_str!("../../../plots/physical-values/main.conduit");
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("physical specimen has no timer effect")
    }
}
#[derive(Default)]
struct Output(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
#[test]
fn reviewed_physical_values_source_executes_exact_receipt_and_comparison() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty());
    assert_eq!(syntax.round_trip(), SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    validate_source(&syntax, &checked).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "physical-values", &profile).unwrap();
    let conversion = authored
        .expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == KIND)
        .unwrap();
    let receipt = prepare_configuration(&conversion.configuration).unwrap();
    validate_receipt(&receipt).unwrap();
    let ConfigurationValue::Quantity(quantity) = &conversion
        .configuration
        .iter()
        .find(|v| v.key == "source")
        .unwrap()
        .value
    else {
        panic!("typed Quantity")
    };
    let ConfigurationValue::Unit(target) = &conversion
        .configuration
        .iter()
        .find(|v| v.key == "to")
        .unwrap()
        .value
    else {
        panic!("typed target Unit")
    };
    assert_eq!(quantity.source(), "1smoot");
    assert_eq!(target.source(), "m");
    assert_eq!(
        quantity.value().convert_to_unit(target.value()),
        Ok((17018, -4))
    );
    let unsupported = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "unsupported".into(),
            boot_id: "boot".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    assert!(conduit_planner::default_expanded_placements(
        &authored.expanded,
        &[unsupported.advertisement().clone()]
    )
    .is_err());
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "physical-values-host".into(),
            boot_id: "physical-values-boot".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_math(),
    );
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [(
        conduit_planner::ForeBoundaryKey {
            direction: PortDirection::Output,
            front_port_id: "result".into(),
            track: ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: MAXIMUM_RECEIPT_BYTES,
        },
    )]
    .into_iter()
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_RECEIPT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.source_document_id, checked.source_document_id);
    let mut output = Output::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[],
            &mut output,
            &mut vec![],
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 1);
    assert!(InfoBool::decode(&output.0[0]).unwrap().get());
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::BackCompleted));
    assert!(kernel.kernel_sign.iter().all(|event| !matches!(
        event.kind,
        conduit_kernel::KernelEventKind::HostCallCompleted
            | conduit_kernel::KernelEventKind::BackFailed
    )));
    let malformed = SOURCE.replace("smoot : Distance", "smoot : Duration");
    let wrong = parse_syntax_document(&malformed);
    let refusal = check_syntax_document(&wrong, &startup).unwrap_err();
    assert!(refusal.span.start < refusal.span.end && refusal.span.end <= malformed.len());
    println!(
        "CONDUIT_PLOT_EVIDENCE={}",
        serde_json::json!({"plan_id":plan.plan_id.as_str(),"play_id":kernel.active_play_id.as_str()})
    );
    println!(
        "PHYSICAL_VALUES_EVIDENCE={}",
        serde_json::json!({"source_document_id":checked.source_document_id.as_str(),"checked_plot_id":checked.plots[0].checked_plot_id.as_str(),"expanded_plot_id":authored.expanded.expanded_plot_id.as_str(),"unit_definition_identity":quantity.value().unit().definition_identity(),"receipt_bytes":receipt.canonical_bytes().unwrap(),"result":true,"malformed_declaration_span":[refusal.span.start,refusal.span.end],"unsupported_math_host":"refused"})
    );
}
