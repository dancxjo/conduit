//! Exact finite two-input take_until, independent of any carrier or device protocol.
use alloc::{vec, vec::Vec};
use conduit_core::*;
pub const FLOW_TAKE_UNTIL_FINITE_KIND: &str = "flow/take-until/finite";
pub fn flow_take_until_finite_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    // Retain the same checked schema eligibility as finite typed state.
    super::seeded_state_semantic_contract(value, schema)?;
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let mut laws = vec![KindSemanticLaw::ValueContracts(
        [
            ("values", FrontValueLocation::Input(port_id("values"))),
            ("taken", FrontValueLocation::Output(port_id("taken"))),
        ]
        .into_iter()
        .map(|(_, location)| FrontValueContract {
            location,
            contract: value.clone(),
        })
        .collect(),
    )];
    laws.push(KindSemanticLaw::ValueContracts(vec![FrontValueContract {
        location: FrontValueLocation::Input(port_id("stop")),
        contract: CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![])
            .map_err(|_| "invalid stop contract")?,
    }]));
    for (input, normal_close) in [
        ("values", NormalCloseTransduction::PropagateAfterDrain),
        ("stop", NormalCloseTransduction::Consume),
    ] {
        laws.push(KindSemanticLaw::TerminalTransduction(
            TerminalTransductionProfile {
                input_port_id: port_id(input),
                output_port_id: port_id("taken"),
                normal_close,
                abnormal: AbnormalTerminalTransduction::NotAccepted,
                cancellation: CancellationTransduction::NotCancellable,
            },
        ));
    }
    Ok(Kind {
        kind_id: kind_id(FLOW_TAKE_UNTIL_FINITE_KIND),
        kind_contract_revision: KindIdentity::from("conduit.flow/take-until-finite@1"),
        startup_parameters: Vec::new(),
        shorthand: None,
        inputs: vec![
            port("values", PortDirection::Input),
            PortDescriptor {
                port_id: port_id("stop"),
                value_kind: kind_id(BOOL_INFO_ID),
                direction: PortDirection::Input,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            },
        ],
        outputs: vec![port("taken", PortDirection::Output)],
        configuration: Vec::new(),
        semantic_laws: laws,
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: value
                .maximum_bytes
                .checked_mul(2)
                .ok_or("finite take_until envelope overflows")?
                .max(1),
        },
    })
}
#[cfg(feature = "plot-catalog")]
pub fn install_flow_take_until_finite_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = flow_take_until_finite_semantic_contract(value, schema)
        .map_err(alloc::string::String::from)?;
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_TAKE_UNTIL_FINITE_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(FLOW_TAKE_UNTIL_FINITE_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}
