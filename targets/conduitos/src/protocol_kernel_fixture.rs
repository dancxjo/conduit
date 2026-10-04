//! Sealed-fore test wrapper: production kernel execution, without a boot claim.
use alloc::vec;
use conduit_composite::*;
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;

pub(crate) fn definition(plan: Plan, mut external: CapabilityOffer) -> KernelCompositeDefinition {
    let fragment = &plan.fragments[0];
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    let mut contracts = vec![];
    for fore in &fragment.fore_ports {
        let gear = fragment
            .placements
            .iter()
            .find(|gear| gear.placement_id == fore.placement_id)
            .unwrap();
        let mut port = if fore.direction == PortDirection::Input {
            &gear.inputs
        } else {
            &gear.outputs
        }
        .iter()
        .find(|port| port.port_id == fore.gear_port_id)
        .unwrap()
        .clone();
        port.port_id = fore.front_port_id.clone();
        if let Some(contract) = &fore.value_contract {
            contracts.push(FrontValueContract {
                location: if fore.direction == PortDirection::Input {
                    FrontValueLocation::Input(fore.front_port_id.clone())
                } else {
                    FrontValueLocation::Output(fore.front_port_id.clone())
                },
                contract: contract.clone(),
            });
        }
        let binding = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: fragment.host_id.clone(),
            internal_placement_id: gear.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(binding);
        } else {
            boundary.output_fronts.push(binding);
        }
    }
    // A sealed-fore fixture wrapper proves kernel execution, not boot admission.
    external.kind_id = kind_id("fixture/protocol-kernel");
    external.kind_contract_revision = KindIdentity::from("fixture/protocol-kernel@1");
    external.capability_id = CapabilityId::from("fixture/protocol-kernel@1");
    external.implementation = ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from("fixture/protocol-kernel@1"),
        implementation_id: ImplementationId::from("fixture/protocol-kernel@1"),
        artifact_id: ArtifactId::from("fixture/protocol-kernel@1"),
    };
    external.semantic_contract = KindSemanticContract {
        configuration: vec![],
        laws: vec![KindSemanticLaw::ValueContracts(contracts)],
    };
    external.inputs = boundary
        .input_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    external.outputs = boundary
        .output_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/protocol-kernel".into(),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    }
}
