use super::{
    state_value_contract, state_value_semantic_contract, STATE_VALUE_KIND, STATE_VALUE_REVISION,
};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    ConfigurationValue, GearId, PlannedStateBoundary, StateContinuation, StateId,
    StructuredInfoType, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_plot::{
    CheckedPlot, KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};

/// Install the kind for a structured type already registered by the caller.
/// A catalogue assembles one exact specialization, as with structured literals.
/// `initial` remains mandatory in authored source; the default initializes only
/// the configuration-field representation required by ProfileCatalog.
pub fn install_state_value_kind(
    type_name: &str,
    value_type: &StructuredInfoType,
    default_value: &StructuredInfoValue,
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    if default_value.value_type() != value_type {
        return Err("State initialization has the wrong exact structured type".into());
    }
    let contract = state_value_semantic_contract(type_name, value_type, default_value)?;
    startup.insert(KindSignature {
        kind: STATE_VALUE_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "initial".into(),
            value_type: type_name.into(),
            default: None,
        }],
    })?;
    profile
        .insert_kind(contract)
        .map_err(|e| alloc::format!("{e:?}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateValueAdmissionError {
    InvalidPlot,
    UnknownGear,
    WrongContract,
    InvalidInitialization,
    InvalidCapacity,
    InitialValueExceedsCapacity,
}

/// Derive State only from the exact authored kind, Front and typed initializer.
/// This is not a migration permission or an effect-authority grant. A host must
/// separately admit its storage, lifetime/evidence resources and implementation.
pub fn derive_state_boundary(
    plot: &CheckedPlot,
    gear_id: &GearId,
    maximum_value_bytes: u32,
) -> Result<PlannedStateBoundary, StateValueAdmissionError> {
    plot.validate_identities()
        .map_err(|_| StateValueAdmissionError::InvalidPlot)?;
    let gear = plot
        .gears
        .iter()
        .find(|gear| &gear.gear_id == gear_id)
        .ok_or(StateValueAdmissionError::UnknownGear)?;
    if gear.kind_id.as_str() != STATE_VALUE_KIND
        || gear.kind_contract_revision.as_str() != STATE_VALUE_REVISION
        || gear.configuration.len() != 1
        || gear.configuration[0].key != "initial"
        || gear.startup_parameters.len() != 1
    {
        return Err(StateValueAdmissionError::WrongContract);
    }
    let ConfigurationValue::Structured(initial) = &gear.configuration[0].value else {
        return Err(StateValueAdmissionError::InvalidInitialization);
    };
    let value = StructuredInfoValue::from_canonical_bytes(initial.canonical_value())
        .map_err(|_| StateValueAdmissionError::InvalidInitialization)?;
    let contract = state_value_contract(
        gear.startup_parameters[0].value_type.as_str(),
        value.value_type(),
    )
    .map_err(|_| StateValueAdmissionError::InvalidInitialization)?;
    if contract.inputs != gear.inputs
        || contract.outputs != gear.outputs
        || contract.startup_parameters != gear.startup_parameters
        || gear.shorthand
            != Some((
                conduit_core::port_id("next"),
                conduit_core::port_id("current"),
            ))
    {
        return Err(StateValueAdmissionError::WrongContract);
    }
    if maximum_value_bytes == 0 || maximum_value_bytes as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
    {
        return Err(StateValueAdmissionError::InvalidCapacity);
    }
    if initial.canonical_value().len() > maximum_value_bytes as usize {
        return Err(StateValueAdmissionError::InitialValueExceedsCapacity);
    }
    Ok(PlannedStateBoundary {
        state_id: StateId::from(gear_id.as_str()),
        gear_id: gear_id.clone(),
        value_kind: contract.outputs[0].value_kind.clone(),
        initial_value: Some(state_payload(&value)?),
        lifetime: conduit_core::StateLifetime::Play,
        retained: None,
        maximum_value_bytes,
        continuation: StateContinuation::ExternallyBounded,
    })
}

/// Validate fresh State initialization against its exact planned semantic Front.
/// Host installation must additionally validate implementation and Boot identity.
/// Migration uses a separate admitted continuity contract, never this fresh path.
pub fn validate_state_placement(
    placement: &conduit_core::PlannedGear,
    state: &PlannedStateBoundary,
) -> Result<(), StateValueAdmissionError> {
    if placement.kind_id.as_str() != STATE_VALUE_KIND
        || placement.kind_contract_revision.as_str() != STATE_VALUE_REVISION
        || placement.gear_id != state.gear_id
        || state.state_id.as_str() != state.gear_id.as_str()
        || state.continuation != StateContinuation::ExternallyBounded
    {
        return Err(StateValueAdmissionError::WrongContract);
    }
    let Some(entry) = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "initial")
    else {
        return Err(StateValueAdmissionError::InvalidInitialization);
    };
    let ConfigurationValue::Structured(initial) = &entry.value else {
        return Err(StateValueAdmissionError::InvalidInitialization);
    };
    let value = StructuredInfoValue::from_canonical_bytes(initial.canonical_value())
        .map_err(|_| StateValueAdmissionError::InvalidInitialization)?;
    let contract = state_value_contract("", value.value_type())
        .map_err(|_| StateValueAdmissionError::InvalidInitialization)?;
    let initial_profile = value
        .value_type()
        .profile()
        .map_err(|_| StateValueAdmissionError::InvalidInitialization)?;
    let expected_initial = state_payload(&value)?;
    if initial.profile() != initial_profile.value_kind()
        || state.value_kind != contract.outputs[0].value_kind
        || state.initial_value.as_deref() != Some(expected_initial.as_slice())
        || placement.inputs != contract.inputs
        || placement.outputs != contract.outputs
    {
        return Err(StateValueAdmissionError::InvalidInitialization);
    }
    let retained_duration = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "retained-duration");
    let maximum = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "maximum-bytes");
    if retained_duration.is_some() != maximum.is_some()
        || placement.configuration.len() != if retained_duration.is_some() { 3 } else { 1 }
    {
        return Err(StateValueAdmissionError::WrongContract);
    }
    if let (Some(duration), Some(maximum)) = (retained_duration, maximum) {
        let ConfigurationValue::Text(duration) = &duration.value else {
            return Err(StateValueAdmissionError::WrongContract);
        };
        let expected_lifetime = match duration.as_str() {
            "step" => conduit_core::StateLifetime::Step,
            "play" => conduit_core::StateLifetime::Play,
            "wake" => conduit_core::StateLifetime::Wake,
            "boot" => conduit_core::StateLifetime::Boot,
            "body" => conduit_core::StateLifetime::Body,
            _ => return Err(StateValueAdmissionError::WrongContract),
        };
        let ConfigurationValue::U64(maximum) = maximum.value else {
            return Err(StateValueAdmissionError::WrongContract);
        };
        if state.lifetime != expected_lifetime || u64::from(state.maximum_value_bytes) != maximum {
            return Err(StateValueAdmissionError::WrongContract);
        }
    } else if state.lifetime != conduit_core::StateLifetime::Play {
        return Err(StateValueAdmissionError::WrongContract);
    }
    if state.maximum_value_bytes == 0
        || state.maximum_value_bytes > placement.limits.max_queue_bytes
        || state.maximum_value_bytes as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
    {
        return Err(StateValueAdmissionError::InvalidCapacity);
    }
    if state
        .initial_value
        .as_ref()
        .is_some_and(|value| value.len() > state.maximum_value_bytes as usize)
    {
        return Err(StateValueAdmissionError::InitialValueExceedsCapacity);
    }
    Ok(())
}

fn state_payload(value: &StructuredInfoValue) -> Result<Vec<u8>, StateValueAdmissionError> {
    match value.shape() {
        conduit_core::StructuredInfoValueShape::Leaf(bytes) => Ok(bytes.to_vec()),
        _ => value
            .canonical_bytes()
            .map_err(|_| StateValueAdmissionError::InvalidInitialization),
    }
}
