//! Explicit bounded temporal lift: one Value becomes one closing Flow item.
use alloc::{vec, vec::Vec};
use conduit_core::*;
pub const FLOW_SINGLETON_KIND: &str = "flow/singleton";
pub const FLOW_SINGLETON_REVISION: &str = "conduit.flow/singleton@1";
pub fn flow_singleton_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    super::seeded_state_semantic_contract(value, schema)?;
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
}
