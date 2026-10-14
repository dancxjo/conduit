//! Authored multi-port mixer graph through ordinary installed Kernel execution.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::{ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, TimerAdapter};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = include_str!("../../../../semantics/audio/dsp/mixer.conduit");
pub(super) struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("mixer has no timer/device effects")
    }
}
#[derive(Default)]
pub(super) struct Outputs(pub(super) Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
pub(super) fn channel_value(
    ty: &StructuredInfoType,
    sample: i32,
    gain: i32,
    timeline: i32,
    frame: i32,
    rate: i32,
) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("channel")
    };
    let values = [
        ("sample", sample),
        ("gain_q8", gain),
        ("timeline", timeline),
        ("frame", frame),
        ("rate_hz", rate),
    ];
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let word = values
                    .iter()
                    .find(|(name, _)| *name == field.name())
                    .unwrap()
                    .1;
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        word.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
pub(super) fn prepare(count: usize) -> (StdHost, Plan, StructuredInfoType, String) {
    let declarations = parse_syntax_document(SOURCE);
    let types = prepare_source_types(&declarations, &StartupCatalog::new()).unwrap();
    let channel = types.named_type("MixerChannel").unwrap().clone();
    let channel_contract =
        CheckedValueContract::new(channel.profile().unwrap().value_kind().clone(), 512, vec![])
            .unwrap();
    let pair_encoder =
        PreparedTypedTuplePairEncoder::new(channel.clone(), 512, channel.clone(), 512).unwrap();
    let pair = pair_encoder.value_type().clone();
    let pair_contract = CheckedValueContract::new(
        pair.profile().unwrap().value_kind().clone(),
        pair_encoder.maximum_bytes(),
        vec![],
    )
    .unwrap();
    let quad = tuple_info_type(vec![pair.clone(), pair.clone()]).unwrap();
    let first = conduit_std_offers::flow_zip_typed_offer(
        &channel_contract,
        &channel,
        &channel_contract,
        &channel,
    )
    .unwrap();
    let second =
        conduit_std_offers::flow_zip_typed_offer(&pair_contract, &pair, &pair_contract, &pair)
            .unwrap();
    let mut profile = ProfileCatalog::new();
    for kind in [
        conduit_semantic_catalog::flow_zip_schema_semantic_contract(
            &channel_contract,
            &channel,
            &channel_contract,
            &channel,
        )
        .unwrap(),
        conduit_semantic_catalog::flow_zip_schema_semantic_contract(
            &pair_contract,
            &pair,
            &pair_contract,
            &pair,
        )
        .unwrap(),
    ] {
        profile.insert_kind(kind).unwrap();
    }
    let mut startup = profile.startup_catalog().unwrap();
    startup
        .insert_structured_type("MixerPair", pair.clone())
        .unwrap();
    startup.insert_structured_type("MixerQuad", quad).unwrap();
    let ports = ["first", "second", "third", "fourth"];
    let mut source = SOURCE.to_string();
    source.push_str("\nplot mixer (\n");
    for port in &ports[..count] {
        source.push_str(&format!(" >> {port}: MixerChannel...| <=512B\n"));
    }
    source.push_str(" result: MixerOutcome...| >>\n) {\n");
    source.push_str(&format!(" pair-a: {}\n", first.kind_id.as_str()));
    source.push_str(" first >> pair-a.left\n second >> pair-a.right\n");
    if count == 4 {
        source.push_str(&format!(
            " pair-b: {}\n all: {}\n sum: dsp/mix-four\n",
            first.kind_id.as_str(),
            second.kind_id.as_str()
        ));
        source.push_str(" third >> pair-b.left\n fourth >> pair-b.right\n pair-a.paired >> all.left\n pair-b.paired >> all.right\n all.paired >> sum.paired\n");
    } else {
        source.push_str(" sum: dsp/mix-two\n pair-a.paired >> sum.paired\n");
    }
    source.push_str(" sum.result >> result\n}\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "mixer", &profile).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement.capabilities.push(first);
    if count == 4 {
        advertisement.capabilities.push(second);
    }
    for gear in &authored.expanded.gears {
        if let Some(ConfigurationEntry {
            value: ConfigurationValue::Text(encoded),
            ..
        }) = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "program")
        {
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            advertisement.capabilities.push(
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .unwrap(),
            );
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, port) in ports[..count]
        .iter()
        .map(|port| (PortDirection::Input, *port))
        .chain([(PortDirection::Output, "result")])
    {
        boundaries.insert(
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: if direction == PortDirection::Input {
                    channel_contract.maximum_bytes
                } else {
                    4096
                },
            },
        );
    }
    let connection_limits = authored
        .expanded
        .connections
        .iter()
        .map(|cord| {
            let choice = &placements.by_gear[&cord.source_gear_id];
            let offer = host
                .advertisement()
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == choice.capability_id)
                .unwrap();
            let location = FrontValueLocation::Output(cord.source_port_id.clone());
            let contract = offer
                .semantic_contract
                .value_contracts()
                .iter()
                .find(|entry| entry.location == location)
                .unwrap();
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: contract.contract.maximum_bytes,
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
        &boundaries,
    )
    .unwrap();
    assert_eq!(
        plan.source_document_id,
        parse_syntax_document(&source).source_document_id()
    );
    (host, plan, channel, source)
}
