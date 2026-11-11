//! Reconstruct numeric identities from the exact sealed Plans, never strings.
use super::*;
use conduit_core::PortDirection;
use conduit_face::{FaceRelationship, FaceRelationshipKind};
use conduit_kernel::debug_observation::DebugSubject;
use conduit_plan_lowering::{
    fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
    lowering::FIXED_KERNEL_STORAGE_PROFILE,
};

pub(super) fn subjects(
    plan: &BodyPlan,
    graphs: &[PatchbayGraph],
    subjects: &mut Vec<FaceSubject>,
    relationships: &mut Vec<FaceRelationship>,
    properties: &mut Vec<FaceProperty>,
) -> Result<Vec<(String, String)>> {
    let plans = plan.plots.iter().map(|plot| &plot.plan).collect::<Vec<_>>();
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(32, 65_536)
        .map_err(|e| format!("historical State profile: {e:?}"))?;
    let lowered = lower_local_fragment_set_from_plans(
        &plans,
        profile,
        FragmentSetBounds {
            fragments: conduit_body::MAX_BODY_PLOTS as u16,
            nodes: 32,
            cords: 64,
            queue_slots: 64,
            value_bytes: 16 * 1024 * 1024,
            sign_items: 4096,
            sign_bytes: 4 * 1024 * 1024,
        },
    )
    .map_err(|e| format!("exact historical Plan lowering: {e:?}"))?;
    let mut mappings = Vec::new();
    for (index, partition) in lowered.partitions.iter().enumerate() {
        let graph = &graphs[index];
        let fragment = &plan.plots[index].plan.fragments[0];
        let map = &partition.identity;
        for (node, placement_id) in &map.placements {
            let placement = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == *placement_id)
                .ok_or("lowered historical placement missing")?;
            let gear = graph
                .gears
                .iter()
                .find(|g| g.gear_id == placement.gear_id)
                .ok_or("actual graph Gear missing")?;
            push(
                &mut mappings,
                DebugSubject::Gear(*node),
                format!("gear/{}", gear.identity),
            )?;
        }
        for port in &map.ports {
            let subject = DebugSubject::Port {
                gear: port.node,
                port: port.port,
            };
            let key = format!("{subject:?}");
            if mappings.iter().any(|(runtime, _)| runtime == &key) {
                continue;
            }
            let placement = fragment
                .placements
                .iter()
                .find(|p| map.node_for_placement(&p.placement_id) == Some(port.node))
                .ok_or("historical port placement missing")?;
            let directional = map
                .ports
                .iter()
                .filter(|p| p.node == port.node && p.port == port.port)
                .map(|p| canonical_port(graph, &placement.gear_id, p.direction, p.port_id.as_str()))
                .collect::<Result<Vec<_>>>()?;
            let canonical = if directional.len() == 1 {
                format!("port/{}", directional[0])
            } else {
                let identity = format!("historical-port/{index}/{}/{}", port.node.0, port.port.0);
                subjects.push(FaceSubject {
                    identity: identity.clone(),
                    role: FaceRole::Port,
                    name: format!(
                        "Observed port {}: combined input/output identity",
                        port.port.0
                    ),
                });
                properties.push(FaceProperty {
                    subject: identity.clone(),
                    name: "direction".into(),
                    value: FacePropertyValue::Text(
                        "combined-input-output; legacy observer omits direction".into(),
                    ),
                });
                for actual in &directional {
                    relationships.push(FaceRelationship {
                        source: identity.clone(),
                        target: format!("port/{actual}"),
                        kind: FaceRelationshipKind::Describes,
                    });
                }
                identity
            };
            push(&mut mappings, subject, canonical)?;
        }
        for (cord, connection_id) in &map.connections {
            // Lowering also lists synthetic external-Fore connections here;
            // bind those through their exact Fore identity below.
            if map
                .fore_endpoints
                .iter()
                .any(|endpoint| endpoint.cord == *cord)
            {
                continue;
            }
            let connection = fragment
                .connections
                .iter()
                .find(|c| c.connection_id == *connection_id)
                .ok_or("actual planned Cord missing")?;
            let source = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == connection.source_placement_id)
                .ok_or("Cord source placement missing")?;
            let sink = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == connection.sink_placement_id)
                .ok_or("Cord sink placement missing")?;
            let source = canonical_port(
                graph,
                &source.gear_id,
                PortDirection::Output,
                connection.source_port_id.as_str(),
            )?;
            let sink = canonical_port(
                graph,
                &sink.gear_id,
                PortDirection::Input,
                connection.sink_port_id.as_str(),
            )?;
            let actual = graph
                .cords
                .iter()
                .find(|c| {
                    c.source_port == source
                        && c.sink_port == sink
                        && c.value_kind == connection.value_kind
                        && c.temporal == connection.temporal
                })
                .ok_or("actual graph Cord endpoints missing")?;
            push(
                &mut mappings,
                DebugSubject::Cord(*cord),
                format!("cord/{}", actual.identity),
            )?;
        }
        for fore in &map.fore_endpoints {
            let key = format!("{:?}", DebugSubject::Cord(fore.cord));
            if mappings.iter().any(|(runtime, _)| runtime == &key) {
                continue;
            }
            let actual = fragment
                .fore_ports
                .iter()
                .find(|p| {
                    p.front_port_id == fore.front_port_id
                        && p.direction == fore.direction
                        && p.track == fore.track
                })
                .ok_or("actual boundary Fore missing")?;
            let placement = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == actual.placement_id)
                .ok_or("Fore placement missing")?;
            let front = match fore.direction {
                PortDirection::Input => &graph.front_inputs,
                PortDirection::Output => &graph.front_outputs,
            }
            .iter()
            .find(|p| p.descriptor.port_id == fore.front_port_id)
            .ok_or("actual graph front missing")?;
            let internal_direction = fore.direction;
            let internal = canonical_port(
                graph,
                &placement.gear_id,
                internal_direction,
                actual.gear_port_id.as_str(),
            )?;
            let (source, sink) = if fore.direction == PortDirection::Input {
                (&front.identity, &internal)
            } else {
                (&internal, &front.identity)
            };
            let actual = graph
                .cords
                .iter()
                .find(|c| {
                    &c.source_port == source
                        && &c.sink_port == sink
                        && c.value_kind == fore.value_kind
                        && c.temporal == actual.temporal
                })
                .ok_or("actual boundary Cord endpoints missing")?;
            push(
                &mut mappings,
                DebugSubject::Cord(fore.cord),
                format!("cord/{}", actual.identity),
            )?;
        }
    }
    for (_, visible) in &mappings {
        if !subjects.iter().any(|s| s.identity == *visible) {
            return Err(format!("actual mapped Face subject absent: {visible}").into());
        }
    }
    Ok(mappings)
}
fn canonical_port(
    graph: &PatchbayGraph,
    gear: &conduit_core::GearId,
    direction: PortDirection,
    port: &str,
) -> Result<String> {
    let gear = graph
        .gears
        .iter()
        .find(|g| &g.gear_id == gear)
        .ok_or("actual graph Gear missing")?;
    let ports = if direction == PortDirection::Input {
        &gear.inputs
    } else {
        &gear.outputs
    };
    Ok(ports
        .iter()
        .find(|p| p.descriptor.port_id.as_str() == port)
        .ok_or("actual directional graph port missing")?
        .identity
        .clone())
}
fn push(mapping: &mut Vec<(String, String)>, subject: DebugSubject, visible: String) -> Result<()> {
    let runtime = format!("{subject:?}");
    if mapping.iter().any(|(prior, _)| prior == &runtime) {
        return Err("reconstructed runtime subject aliases multiple Plan subjects".into());
    }
    mapping.push((runtime, visible));
    Ok(())
}
