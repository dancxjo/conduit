//! One post-Show internal component Source; offer construction is preparation only.
use conduit_core::{
    kind_id, port_id, Back, BackOfferBuilder, CapabilityLimits, CapabilityOffer,
    CheckedValueContract, FrontValueContract, FrontValueLocation, HostCallRequirement, Kind,
    KindSemanticLaw, KindTerminalBehavior, PlannedGear, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_plot::{CheckedNativeType, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_presentation::{
    checked_parsec_input_instruction, PARSEC_INPUT_INSTRUCTION_FRAME_BYTES, SHOW_VALUE_KIND,
};

pub const KIND: &str = "parsec/current-input-instruction";
pub const REVISION: &str = "conduit.parsec/current-input-instruction@1";
pub const IMPLEMENTATION: &str = "std/parsec-current-input-instruction@1";
pub const CONTRACT: &str = "conduit.host/parsec-current-input-instruction@1";
pub const MAXIMUM_SHOW_BYTES: u32 = 65_536;
pub const MAXIMUM_INSTRUCTION_BYTES: u32 = PARSEC_INPUT_INSTRUCTION_FRAME_BYTES as u32;

pub fn instruction_native_type() -> Result<CheckedNativeType, String> {
    // This checked declaration depends only on embedded source. Host offers,
    // reservations and current input custody are validated separately.
    static DECLARATION: std::sync::OnceLock<Result<CheckedNativeType, String>> =
        std::sync::OnceLock::new();
    DECLARATION
        .get_or_init(prepare_instruction_native_type)
        .clone()
}

fn prepare_instruction_native_type() -> Result<CheckedNativeType, String> {
    checked_parsec_input_instruction()
        .map_err(|e| format!("instruction source: {e:?}"))?
        .native_types
        .into_iter()
        .find(|ty| ty.name == "ParsecInputInstructionInfo")
        .ok_or_else(|| "checked instruction output owner is absent".into())
}
pub fn instruction_kind() -> Result<conduit_core::KindId, String> {
    Ok(instruction_native_type()?
        .value_type
        .profile()
        .map_err(|e| format!("{e:?}"))?
        .value_kind()
        .clone())
}
pub fn operation() -> Result<HostCallRequirement, String> {
    Ok(HostCallRequirement {
        contract_id: CONTRACT.into(),
        target_kind: Some(instruction_kind()?),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_SHOW_BYTES,
        maximum_output_bytes: MAXIMUM_INSTRUCTION_BYTES,
    })
}
pub fn semantic_kind() -> Result<Kind, String> {
    let output = instruction_kind()?;
    let port = |name: &str, value_kind, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind,
        direction,
        temporal: if direction == PortDirection::Output {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        },
        abnormal_kind: None,
    };
    Ok(Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(KIND),
        kind_contract_revision: REVISION.into(),
        inputs: vec![port(
            "shown",
            kind_id(SHOW_VALUE_KIND),
            PortDirection::Input,
        )],
        outputs: vec![port("instruction", output.clone(), PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Output(port_id("instruction")),
                contract: CheckedValueContract::new(output, MAXIMUM_INSTRUCTION_BYTES, vec![])
                    .map_err(|e| format!("{e:?}"))?,
            }]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnceWhenScopeIsEligible),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: MAXIMUM_SHOW_BYTES + MAXIMUM_INSTRUCTION_BYTES,
        },
    })
}
pub fn offer() -> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        semantic_kind()?,
        Back {
            capability_id: "std/parsec-current-input-instruction@1".into(),
            execution_profile_id: "conduit.std/parsec-current-input-instruction@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/parsec-current-input-instruction@1".into(),
            host_calls: vec![operation()?],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub fn install_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_checked_native_type("ParsecInputInstructionInfo", &instruction_native_type()?)?;
    startup.insert(KindSignature {
        kind: KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(semantic_kind()?)
        .map_err(|e| format!("{e:?}"))
}
pub(crate) fn validate_placement(placement: &PlannedGear) -> Result<(), String> {
    let expected = offer()?;
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.host_calls != expected.host_calls
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || placement.base.is_some()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.realization_characteristics.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.terminal_transductions.is_empty()
    {
        return Err("instruction source differs from exact selected Back".into());
    }
    Ok(())
}
