//! Ordinary sealed Plot/Plan/std Back/kernel path, with an acknowledged Fore.
use conduit_core::{BaseImplementationId, ConnectionTrack, PortDirection};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost,
    StdHostComposition, StdHostConfig, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};

struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}
#[derive(Default)]
struct Collector(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        Ok(())
    }
}

#[test]
fn ordinary_pipeline_retains_typed_outputs_and_signs() -> Result<(), String> {
    for (gears, alternate) in [(1, false), (8, false), (32, false), (33, false), (33, true)] {
        let source = format!("plot baseline (\n >> input: U64...|\n output: U64...| >>\n) {{\n input{} >> output\n}}\n", (0..gears).map(|i| if alternate && i % 2 == 1 { " >> (. + 2)" } else { " >> (. + 1)" }).collect::<String>());
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new())
            .map_err(|e| format!("check: {e:?}"))?;
        let authoring =
            expand_canonical_plot_for_authoring(&checked, "baseline", &ProfileCatalog::new())
                .map_err(|e| format!("expand: {e:?}"))?;
        assert_eq!(authoring.expanded.gears.len(), gears);
        let base = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "host/checked-capacity".into(),
                boot_id: "boot/checked-capacity".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal(),
        );
        let mut advertisement = base.advertisement().clone();
        for gear in &authoring.expanded.gears {
            let conduit_core::ConfigurationValue::Text(program) = &gear.configuration[0].value
            else {
                panic!("ordinary expression configuration")
            };
            let program = PortableExpressionProgram::from_canonical_hex(program)
                .map_err(|e| format!("program: {e:?}"))?;
            let offer =
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .map_err(|e| format!("offer: {e:?}"))?;
            assert!(gear.accepts_realization(&offer));
            assert_eq!(offer.limits.max_active_instances, 32);
            if !advertisement
                .capabilities
                .iter()
                .any(|e| e.capability_id == offer.capability_id)
            {
                advertisement.capabilities.push(offer);
            }
        }
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let mut host = StdHost::from_advertisement(advertisement)?;
        let hosts = [host.advertisement().clone()];
        let selection = default_expanded_placements(&authoring.expanded, &hosts);
        if gears == 33 && !alternate {
            assert!(
                matches!(
                    selection,
                    Err(conduit_planner::PlannerError::UnknownCapability(_))
                ),
                "33 repeated instances must exceed the offer"
            );
            continue;
        }
        let placements = selection.map_err(|e| format!("selection: {e:?}"))?;
        let boundaries = [
            (PortDirection::Input, "input"),
            (PortDirection::Output, "output"),
        ]
        .into_iter()
        .map(|(direction, port)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 8,
                },
            )
        })
        .collect();
        let plan = plan_expanded_authoring_with_options(
            &authoring,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 8,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(|e| format!("Plan: {e:?}"))?;
        assert!(conduit_core::verify_plan(&plan));
        let inputs = (0_u64..64)
            .map(|i| ExternalForeInput {
                front_port_id: "input".into(),
                track: ConnectionTrack::Payload,
                bytes: i.to_le_bytes().to_vec(),
            })
            .collect::<Vec<_>>();
        let mut collector = Collector(Vec::with_capacity(64));
        let report = host.run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut collector,
            &mut std::io::sink(),
            &mut NoTimer,
        );
        if gears == 33 {
            let refusal = report.expect_err("aggregate profile must refuse 33 nodes");
            assert!(
                refusal.contains("fragment exceeds the installed std kernel profile"),
                "{refusal}"
            );
            assert!(
                collector.0.is_empty(),
                "refusal occurs before any Fore delivery"
            );
            continue;
        }
        let report = report?;
        assert_eq!(collector.0.len(), 64);
        for (index, value) in collector.0.iter().enumerate() {
            assert_eq!(value.sequence, index as u64);
            assert_eq!(value.front_port_id.as_str(), "output");
            assert_eq!(value.track, ConnectionTrack::Payload);
            assert_eq!(value.value_kind, conduit_core::kind_id("value/u64"));
            assert_eq!(value.bytes, (index as u64 + gears as u64).to_le_bytes());
        }
        assert_eq!(report.external_fore_deliveries, collector.0);
        let kernel = report.kernel.unwrap();
        assert!(kernel.decisions > 64);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert_eq!(
            kernel
                .kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count(),
            64
        );
    }
    Ok(())
}
