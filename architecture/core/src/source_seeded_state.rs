//! Exact declared Source-seeded delay boundary; semantic names do not imply state.
use crate::{
    CheckedValueContract, FrontValueLocation, KindSemanticContract, KindSemanticLaw,
    PortDescriptor, PortDirection, PortId, PortTemporal, TemporalStateBehavior,
};

pub struct SourceSeededStateBoundary<'a> {
    pub seed_port_id: &'a PortId,
    pub next_port_id: &'a PortId,
    pub current_port_id: &'a PortId,
    pub value: &'a CheckedValueContract,
}

/// Absence is ordinary retained-state meaning, not permission to cut a cord.
/// A present declaration must have exactly its three typed, bounded ports.
pub fn source_seeded_state_boundary<'a>(
    inputs: &'a [PortDescriptor],
    outputs: &'a [PortDescriptor],
    semantic: &'a KindSemanticContract,
) -> Result<Option<SourceSeededStateBoundary<'a>>, &'static str> {
    let declared = semantic.laws.iter().any(|law| {
        matches!(
            law,
            KindSemanticLaw::TemporalState(
                TemporalStateBehavior::SourceSeededFinite
                    | TemporalStateBehavior::SourceSeededFlowFinite
            )
        )
    });
    if !declared {
        return Ok(None);
    }
    if semantic
        .laws
        .iter()
        .filter(|law| matches!(law, KindSemanticLaw::TemporalState(_)))
        .count()
        != 1
        || inputs.len() != 2
        || outputs.len() != 1
    {
        return Err("Source-seeded state must have one declaration and its exact Fore");
    }
    let port = |ports: &'a [PortDescriptor], name: &str, direction, temporal| {
        ports.iter().find(|port| {
            port.port_id.as_str() == name
                && port.direction == direction
                && port.temporal == temporal
                && port.abnormal_kind.is_none()
        })
    };
    let seed = port(
        inputs,
        "seed",
        PortDirection::Input,
        PortTemporal::Flow { closes: true },
    )
    .ok_or("Source-seeded state has no exact seed Flow")?;
    let next = port(
        inputs,
        "next",
        PortDirection::Input,
        PortTemporal::Flow { closes: true },
    )
    .ok_or("Source-seeded state has no exact next Flow")?;
    let flow = semantic.laws.iter().any(|law| {
        matches!(
            law,
            KindSemanticLaw::TemporalState(TemporalStateBehavior::SourceSeededFlowFinite)
        )
    });
    let current = port(
        outputs,
        "current",
        PortDirection::Output,
        if flow {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Current
        },
    )
    .ok_or("Source-seeded state has no exact committed Current")?;
    let mut values = semantic.laws.iter().filter_map(|law| match law {
        KindSemanticLaw::ValueContracts(values) => Some(values),
        _ => None,
    });
    let values = values
        .next()
        .filter(|values| values.len() == 3)
        .ok_or("Source-seeded state requires exact finite value contracts")?;
    if semantic
        .laws
        .iter()
        .filter(|law| matches!(law, KindSemanticLaw::ValueContracts(_)))
        .count()
        != 1
    {
        return Err("Source-seeded state has duplicate value contracts");
    }
    let contract = |location| {
        values
            .iter()
            .find(|entry| entry.location == location)
            .map(|entry| &entry.contract)
    };
    let value = contract(FrontValueLocation::Input(seed.port_id.clone()))
        .ok_or("Source-seeded state has no seed value contract")?;
    if value.validate_definition().is_err()
        || contract(FrontValueLocation::Input(next.port_id.clone())) != Some(value)
        || contract(FrontValueLocation::Output(current.port_id.clone())) != Some(value)
        || [seed, next, current]
            .iter()
            .any(|port| port.value_kind != value.value_kind)
    {
        return Err("Source-seeded state ports do not share one exact finite contract");
    }
    Ok(Some(SourceSeededStateBoundary {
        seed_port_id: &seed.port_id,
        next_port_id: &next.port_id,
        current_port_id: &current.port_id,
        value,
    }))
}
