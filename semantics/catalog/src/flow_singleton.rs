//! Explicit bounded temporal lift: one Value becomes one closing Flow item.
use alloc::{vec, vec::Vec};
use conduit_core::*;
pub const FLOW_SINGLETON_KIND: &str = "flow/singleton";
pub const FLOW_SINGLETON_REVISION: &str = "conduit.flow/singleton@1";
pub fn flow_singleton_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    if matches!(schema.shape(), StructuredInfoTypeShape::Leaf(kind) if kind != &value.value_kind) {
        // An opaque leaf may travel in its complete canonical structured
        // envelope. Match that exact profile; never substitute a raw leaf ID.
        value
            .validate_definition()
            .map_err(|_| "invalid singleton value contract")?;
        if !value.constraints.is_empty()
            || schema
                .profile()
                .map_err(|_| "invalid singleton schema")?
                .value_kind()
                != &value.value_kind
        {
            return Err("singleton canonical schema differs from its exact contract");
        }
        PreparedStructuredValueValidator::new(schema, value.maximum_bytes as usize)
            .map_err(|_| "singleton schema exceeds its admitted envelope")?;
    } else {
        // Preserve all previously supported raw-leaf/structured contracts.
        super::seeded_state_semantic_contract(value, schema)?;
    }
    let port = |name: &str, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal,
        abnormal_kind: None,
    };
    Ok(Kind {
        kind_id: kind_id(FLOW_SINGLETON_KIND),
        kind_contract_revision: FLOW_SINGLETON_REVISION.into(),
        startup_parameters: Vec::new(),
        shorthand: Some((port_id("value"), port_id("flow"))),
        inputs: vec![port("value", PortDirection::Input, PortTemporal::Value)],
        outputs: vec![port(
            "flow",
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("value")),
                    contract: value.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("flow")),
                    contract: value.clone(),
                },
            ]),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: value.maximum_bytes.max(1),
        },
    })
}
#[cfg(feature = "plot-catalog")]
pub fn install_flow_singleton_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind =
        flow_singleton_semantic_contract(value, schema).map_err(alloc::string::String::from)?;
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_SINGLETON_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(FLOW_SINGLETON_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_schema_and_value_extent_survive_explicit_temporal_lift() {
        let schema = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let value = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
        let kind = flow_singleton_semantic_contract(&value, &schema).unwrap();
        kind.validate().unwrap();
        assert_eq!(kind.inputs[0].temporal, PortTemporal::Value);
        assert_eq!(
            kind.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert!(kind
            .value_contracts()
            .iter()
            .all(|contract| contract.contract == value));
        assert!(flow_singleton_semantic_contract(
            &value,
            &StructuredInfoType::leaf(kind_id("value/u8")).unwrap()
        )
        .is_err());
    }
    fn series_contract() -> (StructuredInfoType, CheckedValueContract) {
        let schema = conduit_data::measurement_plot_series_type();
        let bound = schema.canonical_bytes().unwrap().len()
            + 5
            + conduit_data::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES;
        let value = CheckedValueContract::new(
            schema.profile().unwrap().value_kind().clone(),
            bound as u32,
            vec![],
        )
        .unwrap();
        (schema, value)
    }
    #[test]
    fn canonical_opaque_leaf_keeps_exact_profile_and_value_bound() {
        let (schema, value) = series_contract();
        let kind = flow_singleton_semantic_contract(&value, &schema).unwrap();
        assert_eq!(kind.inputs[0].value_kind, value.value_kind);
        assert_eq!(kind.outputs[0].value_kind, value.value_kind);
        assert!(kind.value_contracts().iter().all(|v| v.contract == value));
        let series = conduit_data::MeasurementPlotSeries::from_projected(
            vec![conduit_data::MeasurementPlotPoint::new(0, 0, 0).unwrap()],
            1,
            0,
        )
        .unwrap();
        let canonical = StructuredInfoValue::leaf(
            schema.clone(),
            conduit_data::encode_measurement_plot_series(&series).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        PreparedStructuredValueValidator::new(&schema, value.maximum_bytes as usize)
            .unwrap()
            .validate(&canonical)
            .unwrap();
    }
    #[test]
    fn canonical_leaf_refuses_a_different_exact_schema() {
        let (_, value) = series_contract();
        assert!(
            flow_singleton_semantic_contract(&value, &conduit_data::measurement_window_type())
                .is_err()
        );
    }
    #[test]
    fn canonical_leaf_does_not_erase_additional_constraints() {
        let (schema, value) = series_contract();
        let constrained = CheckedValueContract::new(
            value.value_kind,
            value.maximum_bytes,
            vec![ValueConstraint::ByteLength {
                minimum: 0,
                maximum: value.maximum_bytes,
            }],
        )
        .unwrap();
        assert!(flow_singleton_semantic_contract(&constrained, &schema).is_err());
    }
}
