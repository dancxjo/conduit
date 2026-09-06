use super::{state_value_contract, STATE_VALUE_KIND, STATE_VALUE_REVISION};
use alloc::{string::String, vec};
use conduit_core::{
    ConfigurationValue, GearId, PlannedStateBoundary, StateContinuation, StateId,
    StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{
    CheckedForm, ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
    ProfileCatalog, StartupCatalog, StartupParameterSignature,
};

/// Install the Kind for a structured type already registered by the caller.
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
    let contract =
        state_value_contract(type_name, value_type).map_err(|e| alloc::format!("{e:?}"))?;
    let initial = StructuredConfigurationValue::new(
        value_type
            .profile()
            .map_err(|e| alloc::format!("{e:?}"))?
            .value_kind()
            .clone(),
        default_value
            .canonical_bytes()
            .map_err(|e| alloc::format!("{e:?}"))?,
    )
    .ok_or_else(|| String::from("invalid finite State initialization"))?;
    startup.insert(KindSignature {
        kind: STATE_VALUE_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "initial".into(),
            value_type: type_name.into(),
            default: None,
        }],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![ConfigurationField {
                key: "initial".into(),
                validation: ConfigurationRule::Structured {
                    profile: initial.profile().clone(),
                },
                default_value: ConfigurationValue::Structured(initial),
            }],
        })
        .map_err(|e| alloc::format!("{e:?}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateValueAdmissionError {
    InvalidForm,
    UnknownGear,
    WrongContract,
    InvalidInitialization,
    InvalidCapacity,
    InitialValueExceedsCapacity,
}

/// Derive State only from the exact authored Kind, Face and typed initializer.
/// This is not a migration permission or an effect-authority grant. A Host must
/// separately admit its storage, lifetime/evidence resources and implementation.
pub fn derive_state_boundary(
    form: &CheckedForm,
    gear_id: &GearId,
    maximum_value_bytes: u32,
) -> Result<PlannedStateBoundary, StateValueAdmissionError> {
    form.validate_identities()
        .map_err(|_| StateValueAdmissionError::InvalidForm)?;
    let gear = form
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
    let contract = state_value_contract(&gear.startup_parameters[0].value_type, value.value_type())
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
        value_kind: initial.profile().clone(),
        initial_value: initial.canonical_value().to_vec(),
        retained: None,
        maximum_value_bytes,
        continuation: StateContinuation::ExternallyBounded,
    })
}

/// Validate fresh State initialization against its exact planned semantic Face.
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
    let [entry] = placement.configuration.as_slice() else {
        return Err(StateValueAdmissionError::InvalidInitialization);
    };
    let ConfigurationValue::Structured(initial) = &entry.value else {
        return Err(StateValueAdmissionError::InvalidInitialization);
    };
    let value = StructuredInfoValue::from_canonical_bytes(initial.canonical_value())
        .map_err(|_| StateValueAdmissionError::InvalidInitialization)?;
    let contract = state_value_contract("", value.value_type())
        .map_err(|_| StateValueAdmissionError::InvalidInitialization)?;
    if entry.key != "initial"
        || initial.profile() != &state.value_kind
        || initial.profile() != &contract.outputs[0].value_kind
        || initial.canonical_value() != state.initial_value
        || placement.inputs != contract.inputs
        || placement.outputs != contract.outputs
    {
        return Err(StateValueAdmissionError::InvalidInitialization);
    }
    if state.maximum_value_bytes == 0
        || state.maximum_value_bytes > placement.limits.max_queue_bytes
        || state.maximum_value_bytes as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
    {
        return Err(StateValueAdmissionError::InvalidCapacity);
    }
    if state.initial_value.len() > state.maximum_value_bytes as usize {
        return Err(StateValueAdmissionError::InitialValueExceedsCapacity);
    }
    Ok(())
}
