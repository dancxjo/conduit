//! One exact Value from each input becomes one typed pair Value.
#[cfg(feature = "plot-catalog")]
use alloc::{format, string::String};
#[cfg(feature = "plot-catalog")]
use conduit_core::PreparedTypedTuplePairEncoder;
use conduit_core::{
    kind_id, CheckedValueContract, Kind, KindSemanticLaw, KindTerminalBehavior, PortTemporal,
    StructuredInfoType,
};
pub const VALUE_PAIR_KIND: &str = "data/pair";
pub const VALUE_PAIR_REVISION: &str = "conduit.data/pair@1";

/// Pairing transports exact semantic Types and payloads; it does not admit a
/// foreign nominal, inspect resource contents, or grant resource authority.
pub fn value_pair_semantic_contract(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    let mut kind = super::flow_zip_finite_semantic_contract(left, left_type, right, right_type)?;
    kind.kind_id = kind_id(VALUE_PAIR_KIND);
    kind.kind_contract_revision = VALUE_PAIR_REVISION.into();
    for port in kind.inputs.iter_mut().chain(&mut kind.outputs) {
        port.temporal = PortTemporal::Value;
    }
    kind.semantic_laws
        .retain(|law| matches!(law, KindSemanticLaw::ValueContracts(_)));
    use conduit_core::{
        ExternalEffectBehavior, ReplayBehavior, SemanticDependence, SuspensionBehavior,
        TemporalStateBehavior, VariabilityBehavior,
    };
    kind.semantic_laws.extend([
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
        KindSemanticLaw::TimeDependence(SemanticDependence::None),
        KindSemanticLaw::RandomDependence(SemanticDependence::None),
        KindSemanticLaw::ResourceDependence(SemanticDependence::None),
        KindSemanticLaw::Suspension(SuspensionBehavior::Never),
        KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
        KindSemanticLaw::Replay(ReplayBehavior::Exact),
    ]);
    kind.semantic_laws
        .push(KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce));
    kind.validate()
        .map_err(|_| "typed Value pair Kind is invalid")?;
    Ok(kind)
}
#[cfg(feature = "plot-catalog")]
pub fn install_value_pair_kind(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let kind =
        value_pair_semantic_contract(left, left_type, right, right_type).map_err(String::from)?;
    let encoder = PreparedTypedTuplePairEncoder::new(
        left_type.clone(),
        left.maximum_bytes,
        right_type.clone(),
        right.maximum_bytes,
    )
    .map_err(|e| format!("pair Type: {e:?}"))?;
    startup.insert(conduit_plot::KindSignature {
        kind: VALUE_PAIR_KIND.into(),
        startup_parameters: alloc::vec![],
    })?;
    startup.insert_fore(VALUE_PAIR_KIND, kind.checked_front())?;
    startup.insert_structured_type(
        format!("{VALUE_PAIR_KIND}/paired"),
        encoder.value_type().clone(),
    )?;
    profile
        .insert_kind(kind)
        .map_err(|e| format!("pair profile: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{FrontValueLocation, PortDirection};
    #[test]
    fn value_pair_has_exact_value_ports_finite_envelopes_and_no_flow_terminal_laws() {
        let left = StructuredInfoType::leaf(kind_id("value/i32")).unwrap();
        let right = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        let left_value = CheckedValueContract::new(kind_id("value/i32"), 4, alloc::vec![]).unwrap();
        let right_value = CheckedValueContract::new(kind_id("value/u8"), 1, alloc::vec![]).unwrap();
        let kind = value_pair_semantic_contract(&left_value, &left, &right_value, &right).unwrap();
        assert_eq!(kind.inputs.len(), 2);
        assert_eq!(kind.outputs.len(), 1);
        assert!(kind
            .inputs
            .iter()
            .chain(&kind.outputs)
            .all(|p| p.temporal == PortTemporal::Value && p.abnormal_kind.is_none()));
        assert!(kind
            .inputs
            .iter()
            .all(|p| p.direction == PortDirection::Input));
        assert!(kind.terminal_transductions().is_empty());
        let output = kind
            .value_contracts()
            .into_iter()
            .find(|v| matches!(v.location, FrontValueLocation::Output(_)))
            .unwrap();
        assert!(output.contract.maximum_bytes > 5);
        assert!(value_pair_semantic_contract(&left_value, &right, &right_value, &right).is_err());
    }
}
