//! A single explicit seed followed by a finite stream of exact replacements.
#[cfg(feature = "plot-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior, PortDescriptor,
    PortDirection, PortTemporal, PreparedStructuredValueValidator, StructuredInfoType,
    StructuredInfoTypeShape, BOOL_INFO_ID,
};

pub const SEEDED_STATE_KIND: &str = "state/seeded/finite";
pub const SEEDED_STATE_REVISION: &str = "conduit.state/seeded-finite@2";

/// Initialization is exactly one ordinary seed value. A replacement presented
/// before the seed waits; after seeding, a second seed is invalid. Replacements
/// publish Current in arrival order under pressure. The replacement stream's
/// close completes the cell, preserving Current rather than inventing a final
/// Flow sample. An empty seed stream fails instead of fabricating initialization.
pub fn seeded_state_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    value
        .validate_definition()
        .map_err(|_| "invalid seeded state value contract")?;
    match schema.shape() {
        StructuredInfoTypeShape::Leaf(kind) if kind == &value.value_kind => {}
        StructuredInfoTypeShape::Leaf(_) => {
            return Err("seeded state schema differs from its contract")
        }
        _ => {
            if schema
                .profile()
                .map_err(|_| "invalid seeded state schema")?
                .value_kind()
                != &value.value_kind
                || !value.constraints.is_empty()
            {
                return Err("unsupported seeded state schema or additional constraints");
            }
            PreparedStructuredValueValidator::new(schema, value.maximum_bytes as usize)
                .map_err(|_| "seeded state schema exceeds its admitted envelope")?;
        }
    }
    let port = |name: &str, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal,
        abnormal_kind: None,
    };
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(SEEDED_STATE_KIND),
        kind_contract_revision: KindIdentity::from(SEEDED_STATE_REVISION),
        inputs: vec![
            port(
                "seed",
                PortDirection::Input,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "next",
                PortDirection::Input,
                PortTemporal::Flow { closes: true },
            ),
        ],
        outputs: vec![port(
            "current",
            PortDirection::Output,
            PortTemporal::Current,
        )],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::TemporalState(conduit_core::TemporalStateBehavior::SourceSeededFinite),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("seed")),
                    contract: value.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("next")),
                    contract: value.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("current")),
                    contract: value.clone(),
                },
            ]),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: value
                .maximum_bytes
                .checked_mul(2)
                .ok_or("seeded state queue envelope overflows")?
                .max(1),
        },
    })
}

pub const SEEDED_STATE_FLOW_KIND: &str = "state/seeded/flow/finite";
pub const SEEDED_STATE_FLOW_REVISION: &str = "conduit.state/seeded-flow-finite@1";

/// Observe the seed and each committed replacement exactly once. Closing next
/// drains these observations and closes current without an extra observation.
/// A finite zip can consume one generation per event without resampling stale Current.
pub fn seeded_state_flow_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    let mut kind = seeded_state_semantic_contract(value, schema)?;
    kind.kind_id = kind_id(SEEDED_STATE_FLOW_KIND);
    kind.kind_contract_revision = KindIdentity::from(SEEDED_STATE_FLOW_REVISION);
    kind.outputs[0].temporal = PortTemporal::Flow { closes: true };
    kind.semantic_laws[0] =
        KindSemanticLaw::TemporalState(conduit_core::TemporalStateBehavior::SourceSeededFlowFinite);
    kind.semantic_laws
        .push(KindSemanticLaw::TerminalTransduction(
            conduit_core::TerminalTransductionProfile {
                input_port_id: port_id("next"),
                output_port_id: port_id("current"),
                normal_close: conduit_core::NormalCloseTransduction::PropagateAfterDrain,
                abnormal: conduit_core::AbnormalTerminalTransduction::NotAccepted,
                cancellation: conduit_core::CancellationTransduction::NotCancellable,
            },
        ));
    Ok(kind)
}

#[cfg(feature = "plot-catalog")]
pub fn install_seeded_state_flow_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = seeded_state_flow_semantic_contract(value, schema).map_err(str::to_string)?;
    startup.insert(conduit_plot::KindSignature {
        kind: SEEDED_STATE_FLOW_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(SEEDED_STATE_FLOW_KIND, kind.checked_front())?;
    profile.insert_kind(kind).map_err(|error| error.to_string())
}

#[cfg(feature = "plot-catalog")]
pub fn install_seeded_state_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let contract = seeded_state_semantic_contract(value, schema).map_err(str::to_string)?;
    startup.insert(conduit_plot::KindSignature {
        kind: SEEDED_STATE_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(SEEDED_STATE_KIND, contract.checked_front())?;
    profile
        .insert_kind(contract)
        .map_err(|error| error.to_string())
}

/// The terminal marker is authored data; the host adds no device termination policy.
pub fn seeded_state_until_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        return Err("terminal state requires a record");
    };
    if !fields.iter().any(|field| field.name() == "terminal" && matches!(field.value_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == BOOL_INFO_ID)) {
        return Err("terminal state requires an exact terminal Boolean");
    }
    let mut kind = seeded_state_flow_semantic_contract(value, schema)?;
    kind.kind_id = kind_id("state/seeded/flow/until");
    kind.kind_contract_revision = KindIdentity::from("conduit.state/seeded-flow-until@1");
    kind.semantic_laws.push(KindSemanticLaw::Terminal(
        KindTerminalBehavior::EmitsThroughSourceTerminalFlag,
    ));
    Ok(kind)
}
#[cfg(feature = "plot-catalog")]
pub fn install_seeded_state_until_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = seeded_state_until_semantic_contract(value, schema).map_err(str::to_string)?;
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profile.insert_kind(kind).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::BOOL_INFO_ID;

    #[test]
    fn seeded_state_contract_preserves_exact_envelopes_and_current_modality() {
        let value = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap();
        let schema = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
        let kind = seeded_state_semantic_contract(&value, &schema).unwrap();
        kind.validate().unwrap();
        assert_eq!(kind.inputs.len(), 2);
        assert_eq!(kind.outputs[0].temporal, PortTemporal::Current);
        assert!(kind
            .inputs
            .iter()
            .all(|port| port.temporal == PortTemporal::Flow { closes: true }
                && port.abnormal_kind.is_none()));
        assert_eq!(kind.semantic_contract().value_contracts().len(), 3);
        assert!(kind
            .semantic_contract()
            .value_contracts()
            .iter()
            .all(|entry| entry.contract == value));
        assert_eq!(kind.limits.max_queue_bytes, 2);
        let mut mismatched = kind;
        mismatched.outputs[0].value_kind = kind_id("wrong");
        assert!(mismatched.validate().is_err());
        assert!(seeded_state_semantic_contract(
            &value,
            &StructuredInfoType::leaf(kind_id("info/text")).unwrap()
        )
        .is_err());
    }
}
