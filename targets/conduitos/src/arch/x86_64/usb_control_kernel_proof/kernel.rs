//! Exact fore wiring and finite kernel preparation for the checked control Plot.
use super::*;
use alloc::vec;
use conduit_composite::*;
use conduit_plot::CompositeFrontTerminal;
pub(super) fn kernel(
    plan: &Plan,
) -> Result<(KernelCompositeHost, PortDescriptor, PortDescriptor), &'static str> {
    if plan.fragments.len() != 1 || plan.fragments[0].placements.len() != 1 {
        return Err("usb-control-proof-shape");
    }
    let fragment = &plan.fragments[0];
    let gear = &fragment.placements[0];
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    for fore in &fragment.fore_ports {
        let ports = if fore.direction == PortDirection::Input {
            &gear.inputs
        } else {
            &gear.outputs
        };
        let mut port = ports
            .iter()
            .find(|port| port.port_id == fore.gear_port_id)
            .ok_or("usb-control-proof-front")?
            .clone();
        port.port_id = fore.front_port_id.clone();
        let binding = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: fragment.host_id.clone(),
            internal_placement_id: fore.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(binding)
        } else {
            boundary.output_fronts.push(binding)
        }
    }
    let input = boundary.input_fronts[0].external_port.clone();
    let output = boundary.output_fronts[0].external_port.clone();
    let mut semantic = gear.semantic_contract.clone();
    for law in &mut semantic.laws {
        if let KindSemanticLaw::ValueContracts(contracts) = law {
            for contract in contracts {
                contract.location = match contract.location {
                    FrontValueLocation::Input(_) => {
                        FrontValueLocation::Input(input.port_id.clone())
                    }
                    FrontValueLocation::Output(_) => {
                        FrontValueLocation::Output(output.port_id.clone())
                    }
                    _ => panic!("control has ordinary input/output contracts"),
                };
            }
        }
    }
    let external = conduit_core::capability_offer_from_parts! {
        semantic_contract:semantic, startup_parameters:vec![],shorthand:None,
        capability_id:"conduitos.proof/usb-control-wrapper".into(), kind_id:gear.kind_id.clone(),kind_contract_revision:gear.kind_contract_revision.clone(),
        implementation:ImplementationOffer {execution_profile_id:"conduitos.proof/usb-wrapper".into(),implementation_id:"conduitos.proof/usb-wrapper".into(),artifact_id:"conduitos.proof/usb-wrapper".into()},
        inputs:vec![input.clone()],outputs:vec![output.clone()],host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],limits:gear.limits.clone(),
    };
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "conduitos.proof/usb-control".into(),
        external_capability: external,
        internal_plan: plan.clone(),
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(
            ControlOperationFactory::prepare_contract().map_err(|_| "usb-control-proof-kernel")?,
        )
        .map_err(|_| "usb-control-proof-kernel")?;
    Ok((
        KernelCompositeHost::prepare(definition, &registry)
            .map_err(|_| "usb-control-proof-kernel")?,
        input,
        output,
    ))
}
