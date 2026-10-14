//! Execute the actual Commons-owned Source through installed std Plan/Play.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = include_str!("../../../plots/commons/main.conduit");
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
        (PortDirection::Output, "places"),
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

fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("Text")
    };
    std::str::from_utf8(bytes).unwrap()
}
#[test]
fn commons_source_places_and_relations_execute_in_installed_plan_play() {
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
    let graph = StructuredInfoValue::from_canonical_bytes(&output.0[0]).unwrap();
    for (slot, identity, name) in [
        ("hub", "commons/hub", "Hub"),
        ("workshop", "commons/workshop", "Workshop"),
        ("creche", "commons/creche", "Crèche"),
    ] {
        let place = field(&graph, slot);
        assert_eq!(text(field(place, "identity")), identity);
        assert_eq!(text(field(place, "name")), name);
        assert_eq!(text(field(place, "role")), "commons/place");
    }
    for (slot, meaning) in [
        ("containment", "commons/contains-place"),
        ("reachability", "commons/reaches-place"),
    ] {
        let StructuredInfoValueShape::Collection(relations) = field(&graph, slot).shape() else {
            panic!("two relations")
        };
        assert_eq!(relations.len(), 2);
        for (relation, target) in relations.iter().zip(["commons/workshop", "commons/creche"]) {
            assert_eq!(text(field(relation, "source")), "commons/hub");
            assert_eq!(text(field(relation, "target")), target);
            assert_eq!(text(field(relation, "meaning")), meaning);
        }
    }
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    let mut refused_output = Outputs::default();
    let refused = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "observe".into(),
            track: ConnectionTrack::Payload,
            bytes: vec![0, 1],
        }],
        &mut refused_output,
        &mut vec![],
        &mut NoTimer,
    );
    assert!(refused.is_err());
    assert!(refused_output.0.is_empty());
    println!(
        "CONDUIT_PLOT_EVIDENCE={}",
        serde_json::json!({"plan_id":plan.plan_id.as_str(),"play_id":kernel.active_play_id.as_str()})
    );
    println!(
        "COMMONS_EXECUTION_EVIDENCE={}",
        serde_json::json!({"source_document_id":plan.source_document_id.as_str(),"checked_plot_id":plan.checked_plot_id.as_str(),"expanded_plot_id":plan.expanded_plot_id.as_str(),"places": ["commons/hub","commons/workshop","commons/creche"],"graph_bytes":output.0[0]})
    );
}
