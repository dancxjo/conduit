//! Source checking contract for the ordinary Crèche operation.
use crate::deployment_runtime::*;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior, PortDescriptor,
    PortDirection, PortTemporal,
};

pub fn install_creche_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.insert(conduit_plot::KindSignature {
        kind: CRECHE_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(creche_kind())
        .map_err(|e| e.to_string())
}
pub fn creche_kind() -> Kind {
    let port = |name: &str, kind: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let contract = |name: &str, kind: &str, bound: usize, input| FrontValueContract {
        location: if input {
            FrontValueLocation::Input(port_id(name))
        } else {
            FrontValueLocation::Output(port_id(name))
        },
        contract: CheckedValueContract::new(kind_id(kind), bound as u32, vec![])
            .expect("finite Creche value bounds"),
    };
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(CRECHE_KIND),
        kind_contract_revision: KindIdentity::from("conduit.creche/prepare@1"),
        inputs: vec![port("request", CRECHE_REQUEST_KIND, PortDirection::Input)],
        outputs: vec![port("outcome", CRECHE_OUTCOME_KIND, PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                contract("request", CRECHE_REQUEST_KIND, CRECHE_REQUEST_BYTES, true),
                contract("outcome", CRECHE_OUTCOME_KIND, CRECHE_OUTCOME_BYTES, false),
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesAfterFixedCount { count: 1 }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: (CRECHE_REQUEST_BYTES + CRECHE_OUTCOME_BYTES) as u32,
        },
    }
}

pub const CRECHE_HOST_CALL: &str = "conduit.host/creche-prepare@1";
pub const CRECHE_IMPLEMENTATION: &str = "std/kernel-creche-prepare@1";
pub const CRECHE_PROFILE: &str = "std/creche-prepare-kernel-hosted@1";
pub const CRECHE_ARTIFACT: &str = "conduit-std-host/creche-prepare@1";
pub const CRECHE_RESOURCE: &str = "conduit.resource/admitted-creche-encounter@1";

/// Publish only when an installed consumer owns an admitted encounter slot.
/// This offer alone does not admit target tools, devices or build authority.
pub fn creche_std_offer() -> conduit_core::CapabilityOffer {
    use conduit_core::*;
    BackOfferBuilder::new(
        creche_kind(),
        Back {
            capability_id: CapabilityId::from("creche-prepare"),
            execution_profile_id: ExecutionProfileId::from(CRECHE_PROFILE),
            implementation_id: ImplementationId::from(CRECHE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(CRECHE_ARTIFACT),
            host_calls: vec![creche_host_call_requirement()],
            resource_requirements: vec![ResourceRequirement {
                content: None,
                class_id: ResourceClassId::from(CRECHE_RESOURCE),
                units: 1,
                protected_role: None,
                compute: None,
            }],
            authority_requirements: vec![],
        },
    )
    .build()
}
pub fn creche_host_call_requirement() -> conduit_core::HostCallRequirement {
    conduit_core::HostCallRequirement {
        contract_id: conduit_core::HostCallContractId::from(CRECHE_HOST_CALL),
        target_kind: Some(conduit_core::kind_id(CRECHE_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: CRECHE_REQUEST_BYTES as u32,
        maximum_output_bytes: CRECHE_OUTCOME_BYTES as u32,
    }
}
pub fn creche_kernel_call_binding(
    call: conduit_kernel::HostCallId,
) -> conduit_kernel::HostCallBinding {
    conduit_kernel::HostCallBinding {
        call,
        maximum_input_bytes: CRECHE_REQUEST_BYTES as u32,
        maximum_output_bytes: CRECHE_OUTCOME_BYTES as u32,
    }
}
