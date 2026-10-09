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
    values: Vec<Vec<u8>>,
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

#[test]
fn installed_quantity_conversion_emits_one_exact_receipt_through_the_shared_kernel() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install(&mut startup, &mut profile).unwrap();
    for (original, target) in [
        ("1kHz", "Hz"),
        ("1µs", "ns"),
        ("1cm²", "mm²"),
        ("0°C", "K"),
        ("30°C", "°F"),
        ("1Qm", "qm"),
        ("1qm", "Qm"),
        ("1°F", "°C"),
        ("1Hz", "m"),
        ("1Qm³", "qm³"),
    ] {
        let source = format!("plot conversion (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {{\n converted: units/convert(source = \"{original}\", to = \"{target}\")\n converted.receipt >> receipt\n}}.\n");
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty());
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        validate_source(&syntax, &checked).unwrap();
        let authored =
            expand_canonical_plot_for_authoring(&checked, "conversion", &profile).unwrap();
        let expected = prepare_configuration(&authored.expanded.gears[0].configuration).unwrap();
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "quantity-host".into(),
                boot_id: "quantity-boot".into(),
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
            "{original} -> {target}"
        );
        validate_receipt(&StructuredInfoValue::from_canonical_bytes(&collector.values[0]).unwrap())
            .unwrap();
        let kernel = report.kernel.unwrap();
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

#[test]
fn quantity_conversion_is_an_optional_math_host_offer() {
    let make = |composition| {
        StdHost::new_with_composition(
            StdHostConfig {
                host_id: "host".into(),
                boot_id: "boot".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            composition,
        )
    };
    let minimal = make(StdHostComposition::minimal());
    assert!(minimal
        .advertisement()
        .capabilities
        .iter()
        .all(|offer| offer.kind_id.as_str() != KIND));
    let math = make(StdHostComposition::minimal().with_math());
    assert_eq!(
        math.advertisement()
            .capabilities
            .iter()
            .filter(|offer| offer.kind_id.as_str() == KIND)
            .count(),
        1
    );
}
