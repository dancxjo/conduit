//! Exact-schema singleton closing Flow to one immutable Value.
use alloc::vec;
use conduit_core::*;
pub const FLOW_EXACTLY_ONE_KIND: &str = "flow/exactly-one";
pub const FLOW_EXACTLY_ONE_REVISION: &str = "conduit.flow/exactly-one@1";
pub fn flow_exactly_one_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    super::seeded_state_semantic_contract(value, schema)?;
    let input = PortDescriptor {
        port_id: port_id("item"),
        value_kind: value.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("value"),
        value_kind: value.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Ok(Kind {
        kind_id: kind_id(FLOW_EXACTLY_ONE_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_EXACTLY_ONE_REVISION),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![input],
        outputs: vec![output],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("item")),
                    contract: value.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("value")),
                    contract: value.clone(),
                },
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: value
                .maximum_bytes
                .checked_mul(2)
                .ok_or("singleton Flow storage overflows")?
                .max(1),
        },
    })
}
#[cfg(feature = "plot-catalog")]
pub fn install_flow_exactly_one_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_EXACTLY_ONE_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(flow_exactly_one_semantic_contract(value, schema).map_err(str::to_string)?)
        .map_err(|error| alloc::format!("{error:?}"))
}
