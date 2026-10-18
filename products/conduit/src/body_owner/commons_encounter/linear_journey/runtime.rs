//! Execute the actual Commons-owned Source through installed std Plan/Play.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = super::super::super::source::SOURCE;
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("no timer effect")
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn prepare(source: &str, root: &str, input: &str, capacity: u32) -> (StdHost, Plan) {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, root, &ProfileCatalog::new()).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear
            .configuration
            .iter()
            .find(|entry| entry.key == "program")
            .unwrap()
            .value
        else {
            panic!("typed program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        advertisement.capabilities.push(
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap(),
        );
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement
        .capabilities
        .dedup_by(|a, b| a.capability_id == b.capability_id);
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, input),
        (
            PortDirection::Output,
            authored.output_bindings[0].front_port_id.as_str(),
        ),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: capacity,
            },
        )
    })
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
            connection_byte_capacity: capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.source_document_id, checked.source_document_id);
    (host, plan)
}

/// Execute the checked Commons graph with finite prepared queues and the real
/// std Host/kernel; this is not the old finite-place Face specimen.
pub(super) fn graph() -> (StructuredInfoValue, serde_json::Value) {
    let (mut host, plan) = prepare(SOURCE, "commons", "observe", 8192);
    let mut output = Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "observe".into(),
                track: ConnectionTrack::Payload,
                bytes: InfoBool::TRUE.encode().to_vec(),
            }],
            &mut output,
            &mut vec![],
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 1);
    assert!(output.0[0].len() <= 8192);
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|e| e.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    let evidence = serde_json::json!({
        "source":plan.source_document_id,"checked":plan.checked_plot_id,
        "expanded":plan.expanded_plot_id,"plan":plan.plan_id,"play":kernel.active_play_id,
        "host":host.advertisement().host_id,"boot":host.advertisement().boot_id,
        "queue_items":1,"queue_bytes":8192,"output_bytes":output.0[0].len(),"graph_bytes":output.0[0],
        "allocation_capacity_before":kernel.value_allocation_capacity_before,
        "allocation_capacity_after":kernel.value_allocation_capacity_after,
        "kernel_sign":kernel.kernel_sign.iter().map(|e| serde_json::json!({
            "sequence":e.sequence,"node":format!("{:?}",e.node),
            "kind":format!("{:?}",e.kind)
        })).collect::<Vec<_>>(),
    });
    (
        StructuredInfoValue::from_canonical_bytes(&output.0[0]).unwrap(),
        evidence,
    )
}
