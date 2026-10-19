//! Physical values keep their admitted definitions through normal installed Play.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = include_str!("../../../plots/physical-values/triad.conduit");
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("no timer effect")
    }
}
#[derive(Default)]
struct Outputs(Vec<conduit_std_host::ExternalForeBytes>);
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
        (PortDirection::Output, "result"),
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
fn execute(host: &mut StdHost, plan: &Plan, port: &str, bytes: Vec<u8>) -> Result<Outputs, String> {
    let mut output = Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut vec![],
            &mut NoTimer,
        )
        .map_err(|error| format!("{error:?}"))?;
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    Ok(output)
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a [u8] {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("triad")
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .shape()
    else {
        panic!("physical leaf")
    };
    bytes
}
#[test]
fn physical_triad_arguments_fields_connections_and_old_play_keep_original_definition() {
    let (mut host, plan) = prepare(SOURCE, "demo", "tick", 8192);
    let changed = SOURCE.replace("reference: origin, scale: 1", "reference: origin, scale: 2");
    let (mut later, later_plan) = prepare(&changed, "demo", "tick", 8192);
    let newer = execute(
        &mut later,
        &later_plan,
        "tick",
        InfoBool::TRUE.encode().to_vec(),
    )
    .unwrap();
    let prior = execute(&mut host, &plan, "tick", InfoBool::TRUE.encode().to_vec()).unwrap();
    assert_eq!(prior.0.len(), 1);
    assert_eq!(newer.0.len(), 1);
    assert_ne!(prior.0, newer.0);
    let old = StructuredInfoValue::from_canonical_bytes(&prior.0[0]).unwrap();
    let new = StructuredInfoValue::from_canonical_bytes(&newer.0[0]).unwrap();
    let dimension = Dimension::decode(field(&old, "dimension")).unwrap();
    assert_eq!(
        dimension,
        Dimension::basis("travel")
            .unwrap()
            .quotient(Dimension::basis("clock").unwrap())
            .unwrap()
    );
    let unit = Unit::decode(field(&old, "coordinate")).unwrap();
    let quantity = Quantity::decode(field(&old, "magnitude")).unwrap();
    assert_eq!(quantity.convert_to_unit(unit), Ok((3, 0)));
    assert_eq!(
        unit.definition_identity(),
        quantity.unit().definition_identity()
    );
    let (mut alternate, alternate_plan) = prepare(SOURCE, "demo-other", "tick", 8192);
    let alternate = execute(
        &mut alternate,
        &alternate_plan,
        "tick",
        InfoBool::TRUE.encode().to_vec(),
    )
    .unwrap();
    let alternate = StructuredInfoValue::from_canonical_bytes(&alternate.0[0]).unwrap();
    let same_magnitude = Quantity::decode(field(&alternate, "magnitude")).unwrap();
    assert_ne!(quantity.encode(), same_magnitude.encode());
    assert_eq!(
        quantity.compare(same_magnitude),
        Ok(core::cmp::Ordering::Equal)
    );
    let later_unit = Unit::decode(field(&new, "coordinate")).unwrap();
    assert_ne!(unit.definition_identity(), later_unit.definition_identity());
    let encoded_dimension = dimension.encode();
    let mut forged_dimension = encoded_dimension;
    forged_dimension[2] ^= 1;
    assert!(Dimension::decode(&forged_dimension).is_err());
    let offset = prior.0[0]
        .windows(encoded_dimension.len())
        .position(|bytes| bytes == encoded_dimension)
        .unwrap();
    let mut forged = prior.0[0].to_vec();
    forged[offset + 2] ^= 1;
    for (name, bytes) in [
        ("forged dimension metadata", forged),
        ("wrong kind", quantity.encode().to_vec()),
        ("profile overflow", vec![0; 8193]),
    ] {
        let (mut refused, refused_plan) = prepare(SOURCE, "relay", "value", 8192);
        let mut output = Outputs::default();
        let result = refused.run_external_plot_to(
            refused_plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut vec![],
            &mut NoTimer,
        );
        assert!(result.is_err(), "{name}: {result:?}");
        assert!(output.0.is_empty());
    }
    // Re-admit the old record through a separately planned runtime input Fore.
    let (mut relay, relay_plan) = prepare(&changed, "relay", "value", 8192);
    assert_eq!(
        execute(&mut relay, &relay_plan, "value", prior.0[0].to_vec())
            .unwrap()
            .0,
        prior.0
    );
}
