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
            self.0.push(value.bytes.to_vec());
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
        serde_json::json!({"host_id":host.advertisement().host_id.as_str(),"boot_id":host.advertisement().boot_id.as_str(),"source_document_id":plan.source_document_id.as_str(),"checked_plot_id":plan.checked_plot_id.as_str(),"expanded_plot_id":plan.expanded_plot_id.as_str(),"places": ["commons/hub","commons/workshop","commons/creche"],"graph_byte_count":output.0[0].len(),"graph_bytes":output.0[0]})
    );
}

fn encounter_input(
    graph: &StructuredInfoValue,
    body: &str,
    life: &str,
    current: &str,
    target: &str,
    back: bool,
) -> Vec<u8> {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let request = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "CommonsEncounterRequest")
        .unwrap()
        .value_type;
    fn member<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            panic!("record");
        };
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value_type()
    }
    fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> StructuredInfoValue {
        if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
            return StructuredInfoValue::nominal(ty.clone(), leaf(representation, bytes)).unwrap();
        }
        StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).unwrap()
    }
    fn record(
        ty: &StructuredInfoType,
        fields: Vec<(&str, StructuredInfoValue)>,
    ) -> StructuredInfoValue {
        StructuredInfoValue::record(
            ty.clone(),
            fields
                .into_iter()
                .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
                .collect(),
        )
        .unwrap()
    }
    let owner_ty = member(request, "owner");
    let owner = record(
        owner_ty,
        vec![
            (
                "body_id",
                leaf(member(owner_ty, "body_id"), body.as_bytes()),
            ),
            ("life", leaf(member(owner_ty, "life"), life.as_bytes())),
            (
                "revision",
                leaf(member(owner_ty, "revision"), &7_u64.to_le_bytes()),
            ),
        ],
    );
    record(
        request,
        vec![
            ("places", graph.clone()),
            ("owner", owner),
            (
                "current",
                leaf(member(request, "current"), current.as_bytes()),
            ),
            ("target", leaf(member(request, "target"), target.as_bytes())),
            (
                "back",
                leaf(member(request, "back"), &InfoBool::new(back).encode()),
            ),
        ],
    )
    .canonical_bytes()
    .unwrap()
}
fn execute(host: &mut StdHost, plan: &Plan, port: &str, bytes: Vec<u8>) -> Result<Outputs, String> {
    let mut outputs = Outputs::default();
    let report = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: port.into(),
            track: ConnectionTrack::Payload,
            bytes,
        }],
        &mut outputs,
        &mut vec![],
        &mut NoTimer,
    )?;
    let kernel = report.kernel.ok_or("missing kernel")?;
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    println!(
        "COMMONS_ENCOUNTER_EXECUTION={}",
        serde_json::json!({
            "source":plan.source_document_id,"checked":plan.checked_plot_id,"expanded":plan.expanded_plot_id,
            "plan":plan.plan_id,"play":kernel.active_play_id,"host":host.advertisement().host_id,
            "boot":host.advertisement().boot_id,"terminal_sign_id":null,
            "kernel_sign":kernel.kernel_sign.iter().map(|event| serde_json::json!({
                "sequence":event.sequence,"node":format!("{:?}",event.node),
                "port":format!("{:?}",event.port),"request":format!("{:?}",event.request),
                "kind":format!("{:?}",event.kind)
            })).collect::<Vec<_>>(),
        })
    );
    Ok(outputs)
}
#[test]
fn commons_encounter_observation_and_navigation_execute_through_std_plan_play() {
    let (mut host, plan) = prepare(SOURCE, "commons", "observe", 8192);
    let graph = execute(
        &mut host,
        &plan,
        "observe",
        InfoBool::TRUE.encode().to_vec(),
    )
    .unwrap();
    let graph = StructuredInfoValue::from_canonical_bytes(&graph.0[0]).unwrap();
    let (mut host, plan) = prepare(SOURCE, "commons/encounter", "request", 32768);
    for (body, life, offers) in [
        ("", "none", false),
        ("retained/body", "lulled", true),
        ("retained/body", "awake", true),
        ("retained/body", "fulfilled", false),
        ("", "unknown", false),
    ] {
        for target in ["commons/workshop", "commons/creche"] {
            for refusal in ["places", "owner", "navigation"] {
                let (mut admission_host, admission_plan) = prepare(
                    SOURCE,
                    &format!("commons/refuse-invalid-{refusal}"),
                    "request",
                    32768,
                );
                let admitted = execute(
                    &mut admission_host,
                    &admission_plan,
                    "request",
                    encounter_input(&graph, body, life, "commons/hub", target, false),
                )
                .unwrap();
                assert!(
                    !InfoBool::decode(&admitted.0[0]).unwrap().get(),
                    "{refusal}"
                );
            }
            let output = execute(
                &mut host,
                &plan,
                "request",
                encounter_input(&graph, body, life, "commons/hub", target, false),
            )
            .unwrap();
            let encounter = StructuredInfoValue::from_canonical_bytes(&output.0[0]).unwrap();
            assert_eq!(text(field(&encounter, "selected")), target);
            assert_eq!(text(field(field(&encounter, "owner"), "body_id")), body);
            assert_eq!(text(field(field(&encounter, "owner"), "life")), life);
            assert_eq!(field(&encounter, "places"), &graph);
            let StructuredInfoValueShape::Leaf(value) = field(&encounter, "body_actions").shape()
            else {
                panic!("boolean")
            };
            assert_eq!(
                value,
                &InfoBool::new(offers && target != "commons/creche").encode()
            );
            let returned = execute(
                &mut host,
                &plan,
                "request",
                encounter_input(&graph, body, life, target, "commons/hub", true),
            )
            .unwrap();
            let returned = StructuredInfoValue::from_canonical_bytes(&returned.0[0]).unwrap();
            assert_eq!(text(field(&returned, "selected")), "commons/hub");
        }
    }
    for (body, life, current, target, back) in [
        (
            "retained/body",
            "lulled",
            "commons/workshop",
            "commons/hub",
            false,
        ),
        (
            "retained/body",
            "awake",
            "commons/workshop",
            "commons/creche",
            false,
        ),
        (
            "retained/body",
            "awake",
            "commons/hub",
            "commons/creche",
            true,
        ),
        ("forged", "none", "commons/hub", "commons/workshop", false),
        ("", "lulled", "commons/hub", "commons/workshop", false),
        ("", "none", "commons/hub", "commons/invented", false),
    ] {
        let refusal =
            if (life == "none" && !body.is_empty()) || (life == "lulled" && body.is_empty()) {
                "owner"
            } else {
                "navigation"
            };
        let (mut host, plan) = prepare(
            SOURCE,
            &format!("commons/refuse-invalid-{refusal}"),
            "request",
            32768,
        );
        let mut outputs = Outputs::default();
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: encounter_input(&graph, body, life, current, target, back),
            }],
            &mut outputs,
            &mut vec![],
            &mut NoTimer,
        );
        result.unwrap();
        assert_eq!(outputs.0.len(), 1);
        assert!(
            InfoBool::decode(&outputs.0[0]).unwrap().get(),
            "{life}/{current}/{target}/{back}: {refusal}"
        );
    }
}
