//! Preparation-only exact checked Source/Fore/Back selection.
use super::*;
use conduit_core::*;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{
    expand_canonical_plot_for_authoring, PortableExpressionProgram, ProfileCatalog,
};
use std::collections::BTreeMap;

pub(super) fn offers(
    document: &CheckedSyntaxDocument,
    names: &[&str],
) -> Result<Vec<CapabilityOffer>, DriverRefusal> {
    let mut offers = Vec::new();
    for name in names {
        let authored = expand_canonical_plot_for_authoring(document, name, &ProfileCatalog::new())
            .map_err(|error| DriverRefusal::PreparationDetail {
                stage: "source-plan",
                detail: format!("{name}: {error:?}"),
            })?;
        for gear in &authored.expanded.gears {
            if gear.inputs.len() != 1 || gear.outputs.len() != 1 || gear.configuration.len() != 1 {
                return Err(DriverRefusal::PreparationDetail {
                    stage: "unsupported-source-shape",
                    detail: name.to_string(),
                });
            }
            let entry = &gear.configuration[0];
            let ConfigurationValue::Text(encoded) = &entry.value else {
                return Err(DriverRefusal::PreparationDetail {
                    stage: "unsupported-source-shape",
                    detail: name.to_string(),
                });
            };
            if entry.key != "program" {
                return Err(DriverRefusal::PreparationDetail {
                    stage: "unsupported-source-shape",
                    detail: name.to_string(),
                });
            }
            let program =
                PortableExpressionProgram::from_canonical_hex(encoded).map_err(|error| {
                    DriverRefusal::PreparationDetail {
                        stage: "source-plan",
                        detail: format!("{name}: {error:?}"),
                    }
                })?;
            let offer = conduit_std_offers::pure_expression_local_flow_offer(&program).map_err(
                |error| DriverRefusal::PreparationDetail {
                    stage: "source-plan",
                    detail: format!("{name}: {error:?}"),
                },
            )?;
            if !offers.contains(&offer) {
                offers.push(offer);
            }
        }
    }
    Ok(offers)
}
fn contract(
    offer: &CapabilityOffer,
    port: &PortId,
    direction: PortDirection,
    program_bounds: (u32, u32),
) -> Result<u32, DriverRefusal> {
    let location = match direction {
        PortDirection::Input => FrontValueLocation::Input(port.clone()),
        PortDirection::Output => FrontValueLocation::Output(port.clone()),
    };
    let (ports, derived) = match direction {
        PortDirection::Input => (&offer.inputs, program_bounds.0),
        PortDirection::Output => (&offer.outputs, program_bounds.1),
    };
    if !ports.iter().any(|entry| &entry.port_id == port) {
        return Err(DriverRefusal::PreparationDetail {
            stage: "source-plan-port",
            detail: format!("missing advertised port: {location:?}"),
        });
    }
    // ValueContracts narrow selected ports; they are not present for every
    // ordinary expression. Keep the actual program's prepared structural bound.
    Ok(offer
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == location)
        .map_or(derived, |entry| derived.max(entry.contract.maximum_bytes)))
}
pub(super) fn definition(
    document: &CheckedSyntaxDocument,
    host: &HostAdvertisement,
    name: &str,
) -> Result<KernelCompositeDefinition, DriverRefusal> {
    let catalog = ProfileCatalog::new();
    let authored =
        expand_canonical_plot_for_authoring(document, name, &catalog).map_err(|error| {
            DriverRefusal::PreparationDetail {
                stage: "source-plan",
                detail: format!("{name}: {error:?}"),
            }
        })?;
    if authored.input_bindings.len() != 1 || authored.output_bindings.len() != 1 {
        return Err(DriverRefusal::PreparationDetail {
            stage: "unsupported-source-shape",
            detail: name.to_string(),
        });
    }
    let hosts = [host.clone()];
    let placements = conduit_planner::default_expanded_placements(&authored.expanded, &hosts)
        .map_err(|error| DriverRefusal::PreparationDetail {
            stage: "source-plan",
            detail: format!("{name}: {error:?}"),
        })?;
    let selected = |gear: &GearId| -> Result<&CapabilityOffer, DriverRefusal> {
        let choice =
            placements
                .by_gear
                .get(gear)
                .ok_or_else(|| DriverRefusal::PreparationDetail {
                    stage: "source-plan-port",
                    detail: "missing selected port/offer".into(),
                })?;
        host.capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| DriverRefusal::PreparationDetail {
                stage: "source-plan-port",
                detail: "missing selected port/offer".into(),
            })
    };
    let mut program_bounds = BTreeMap::new();
    for gear in &authored.expanded.gears {
        let [entry] = gear.configuration.as_slice() else {
            return Err(DriverRefusal::Preparation);
        };
        let ConfigurationValue::Text(encoded) = &entry.value else {
            return Err(DriverRefusal::Preparation);
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).map_err(|error| {
            DriverRefusal::PreparationDetail {
                stage: "source-value-bound",
                detail: format!("{name}: {error:?}"),
            }
        })?;
        let input = program.maximum_prepared_input_bytes().map_err(|error| {
            DriverRefusal::PreparationDetail {
                stage: "source-input-bound",
                detail: format!("{name}: {error:?}"),
            }
        })?;
        let output = program.maximum_prepared_output_bytes().map_err(|error| {
            DriverRefusal::PreparationDetail {
                stage: "source-output-bound",
                detail: format!("{name}: {error:?}"),
            }
        })?;
        program_bounds.insert(gear.gear_id.clone(), (input, output));
    }
    let mut bounds = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, &authored.input_bindings),
        (PortDirection::Output, &authored.output_bindings),
    ] {
        for binding in bindings {
            bounds.insert(
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: contract(
                        selected(&binding.gear_id)?,
                        &binding.gear_port_id,
                        direction,
                        program_bounds[&binding.gear_id],
                    )?,
                },
            );
        }
    }
    let mut cords = BTreeMap::new();
    for cord in &authored.expanded.connections {
        let bytes = contract(
            selected(&cord.source_gear_id)?,
            &cord.source_port_id,
            PortDirection::Output,
            program_bounds[&cord.source_gear_id],
        )?;
        cords.insert(
            (
                cord.source_gear_id.clone(),
                cord.source_port_id.clone(),
                cord.sink_gear_id.clone(),
                cord.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        );
    }
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            // Every actual internal connection and Fore has an explicit contract bound.
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &cords,
        &bounds,
    )
    .map_err(|error| DriverRefusal::PreparationDetail {
        stage: "source-plan",
        detail: format!("{name}: {error:?}"),
    })?;
    KernelCompositeDefinition::from_canonical_export(
        host.host_id.clone(),
        host.boot_id.clone(),
        host.offer_generation,
        host.profile.clone(),
        "language/analysis-round-local@1".into(),
        "language/analysis-round-local-source@1".into(),
        document,
        name,
        &catalog,
        plan,
        FailureReason::CompositeCapabilityFailed,
    )
    .map_err(|error| DriverRefusal::PreparationDetail {
        stage: "canonical-export",
        detail: format!("{name}: {error:?}"),
    })
}
