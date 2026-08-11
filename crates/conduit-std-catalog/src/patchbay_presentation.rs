//! Canonical Patchbay presentation waist and first subject-specific Backs.

use super::{StandardKindContract, TerminalBehavior};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, ImplementationId,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal, PRESENTATION_RESOURCE_CLASS,
};

pub const PATCHBAY_PRESENTATION_KIND: &str = "presentation/patchbay";
pub const PATCHBAY_GEAR_FACE_KIND: &str = "patchbay/gear-face";
pub const PATCHBAY_PORT_KIND: &str = "patchbay/port";
pub const PATCHBAY_CORD_KIND: &str = "patchbay/cord";
pub const PATCHBAY_PRESENTATION_REVISION: &str = "conduit.patchbay/presentation@1";
pub const PATCHBAY_PRESENTATION_INPUT: &str = "subject";
pub const PATCHBAY_PRESENTATION_VALUE_KIND: &str = "value/text@1";
pub const MAX_PATCHBAY_PRESENTATION_BYTES: u32 = 1_024;

pub fn patchbay_presentation_contracts() -> [StandardKindContract; 4] {
    [
        contract(
            PATCHBAY_PRESENTATION_KIND,
            "Patchbay presentation",
            "Present one bounded normalized Patchbay meaning without owning its semantic subjects.",
        ),
        contract(
            PATCHBAY_GEAR_FACE_KIND,
            "Patchbay Gear Face",
            "Present one existing Gear subject with exact label, Ports, and semantic controls.",
        ),
        contract(
            PATCHBAY_PORT_KIND,
            "Patchbay Port",
            "Present one existing typed Port subject with exact direction and accessible name.",
        ),
        contract(
            PATCHBAY_CORD_KIND,
            "Patchbay Cord",
            "Present one existing Cord subject while keeping active Line truth an annotation.",
        ),
    ]
}

pub fn patchbay_presentation_offers() -> [CapabilityOffer; 4] {
    patchbay_presentation_contracts().map(offer)
}

fn contract(kind: &str, name: &str, summary: &str) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id(PATCHBAY_PRESENTATION_INPUT),
            value_kind: kind_id(PATCHBAY_PRESENTATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: MAX_PATCHBAY_PRESENTATION_BYTES,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: kind == PATCHBAY_PRESENTATION_KIND,
        pico_manifestation_honest: false,
        example: alloc::format!("subject: text/literal(\"bounded normalized subject\") > {kind}"),
    }
}

fn offer(contract: StandardKindContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!("patchbay/{kind}-direct@1")),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(PATCHBAY_PRESENTATION_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("patchbay/presenter-kernel-hosted@1"),
            implementation_id: ImplementationId::from(alloc::format!(
                "patchbay/direct/{}@1",
                kind.replace('/', "-")
            )),
            artifact_id: ArtifactId::from("patchbay-model/direct-presentation@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![present_host_operation_requirement(
            kind_id("presentation/patchbay-surface@1"),
            MAX_PATCHBAY_PRESENTATION_BYTES,
        )],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_patchbay_presentation_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};
    for contract in patchbay_presentation_contracts() {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(PATCHBAY_PRESENTATION_REVISION),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_canonical_family_carries_subject_text_not_widget_or_renderer_types() {
        for (contract, offer) in patchbay_presentation_contracts()
            .into_iter()
            .zip(patchbay_presentation_offers())
        {
            assert_eq!(contract.kind_id, offer.kind_id);
            assert_eq!(contract.inputs, offer.inputs);
            assert_eq!(contract.outputs, offer.outputs);
            assert_eq!(contract.inputs[0].value_kind.as_str(), "value/text@1");
            let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
            for forbidden in ["widget", "dom", "css", "framebuffer", "socket"] {
                assert!(!rendered.contains(forbidden));
            }
        }
    }
}
