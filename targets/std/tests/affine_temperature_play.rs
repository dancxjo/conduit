//! Authored Source → exact host offer → plan → installed preparation → kernel receipt.
use conduit_core::{BaseImplementationId, ConnectionTrack, PortDirection, StructuredInfoValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    quantity_conversion::*, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
    TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};

struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("conversion has no timer effect");
    }
}
#[derive(Default)]
struct Collector {
    values: Vec<conduit_std_host::ExternalForeBytes>,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        match delivery.track {
            ConnectionTrack::Payload => self.values.push(delivery.bytes),
            ConnectionTrack::NormalClose => {}
            _ => panic!("no abnormal conversion effect"),
        }
        Ok(())
    }
}

const DECLARATIONS: &str =
    include_str!("../../../architecture/plot/tests/fixtures/affine_temperature.conduit");
#[test]
fn source_owned_affine_points_and_deltas_execute_in_installed_plan_play() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install(&mut startup, &mut profile).unwrap();
    for (value, target) in [
        ("0celsiusPoint", "kelvinPoint"),
        ("32fahrenheitPoint", "celsiusPoint"),
        ("30celsiusPoint", "fahrenheitPoint"),
        ("ThermalDelta(9, fahrenheitPoint)", "kelvinPoint"),
        ("ThermalDelta(1, celsiusPoint)", "fahrenheitPoint"),
    ] {
        let source = format!("{DECLARATIONS}\nplot conversion (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {{\n convert: units/convert(source = {value}, to = {target})\n convert.receipt >> receipt\n}}\n");
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        let authored =
            expand_canonical_plot_for_authoring(&checked, "conversion", &profile).unwrap();
        let expected = prepare_configuration(&authored.expanded.gears[0].configuration).unwrap();
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "affine-host".into(),
                boot_id: "affine-boot".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal().with_math(),
        );
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = [(
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: "receipt".into(),
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
        let mut collector = Collector::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[],
                &mut collector,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(
            collector.values,
            [expected.canonical_bytes().unwrap()],
            "{value} -> {target}"
        );
        let receipt = StructuredInfoValue::from_canonical_bytes(&collector.values[0]).unwrap();
        validate_receipt(&receipt).unwrap();
        let kernel = report.kernel.unwrap();
        eprintln!(
            "AFFINE_TEMPERATURE_EVIDENCE={}",
            serde_json::json!({
                "value":value, "target":target,
                "source_id":plan.source_document_id.as_str(),
                "checked_id":plan.checked_plot_id.as_str(),
                "expanded_id":plan.expanded_plot_id.as_str(),
                "plan_id":plan.plan_id.as_str(), "play_id":kernel.active_play_id.as_str()
            })
        );

        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|sign| sign.kind == conduit_kernel::KernelEventKind::BackCompleted));
        assert!(kernel.kernel_sign.iter().all(|sign| !matches!(
            sign.kind,
            conduit_kernel::KernelEventKind::HostCallRequested
                | conduit_kernel::KernelEventKind::HostCallCancellationRequested
                | conduit_kernel::KernelEventKind::HostCallCompleted
                | conduit_kernel::KernelEventKind::BackFailed
        )));
    }
}
