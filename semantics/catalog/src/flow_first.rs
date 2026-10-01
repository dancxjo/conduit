//! Exact finite first-value arbitration over one checked value specialization.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior, PortDescriptor,
    PortDirection, PortTemporal, CANCELLATION_REQUEST_INFO_ID,
};

use crate::{StandardKindContract, FIRST_KIND, LEFT_PORT, OUT_PORT, RIGHT_PORT};

pub const FLOW_FIRST_CONTRACT_REVISION: &str = "conduit.std/flow-first@1";
pub const CANCEL_LEFT_PORT: &str = "cancel-left";
pub const CANCEL_RIGHT_PORT: &str = "cancel-right";
pub const FLOW_FIRST_MAXIMUM_PENDING_VALUES: u16 = 2;

pub fn flow_first_scalar_contract() -> StandardKindContract {
    let value = CheckedValueContract::new(
        kind_id(conduit_core::SCALAR_INFO_ID),
        conduit_core::SCALAR_ENCODED_LEN as u32,
        Vec::new(),
    )
    .expect("Scalar has one canonical finite envelope");
    let kind = flow_first_contract(&value).expect("Scalar specializes flow/first");
    StandardKindContract {
        kind_id: kind.kind_id,
        plain_name: "First scalar flow".to_string(),
        summary: "Emit the first observed scalar exactly once; left wins an exact simultaneous tie and the loser receives one cancellation request.".to_string(),
        inputs: kind.inputs,
        outputs: kind.outputs,
        configuration: kind.configuration,
        limits: kind.limits,
        terminal_behavior:
            KindTerminalBehavior::FirstReadyLeftTieCancelsLoserOrCompletesWithoutWinner,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "winner: flow/first".to_string(),
    }
}

pub fn flow_first_scalar_semantic_contract() -> Kind {
    flow_first_contract(
        &CheckedValueContract::new(
            kind_id(conduit_core::SCALAR_INFO_ID),
            conduit_core::SCALAR_ENCODED_LEN as u32,
            Vec::new(),
        )
        .expect("Scalar has one canonical finite envelope"),
    )
    .expect("Scalar specializes flow/first")
}

pub fn flow_first_contract(value: &CheckedValueContract) -> Result<Kind, &'static str> {
    if value.maximum_bytes == 0 {
        return Err("flow/first requires one finite nonzero value envelope");
    }
    let payload = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal: if direction == PortDirection::Input {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        },
        abnormal_kind: None,
    };
    let cancellation = |name: &str| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(CANCELLATION_REQUEST_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let inputs = vec![
        payload(LEFT_PORT, PortDirection::Input),
        payload(RIGHT_PORT, PortDirection::Input),
    ];
    let outputs = vec![
        payload(OUT_PORT, PortDirection::Output),
        cancellation(CANCEL_LEFT_PORT),
        cancellation(CANCEL_RIGHT_PORT),
    ];
    let value_contracts = [
        (FrontValueLocation::Input(port_id(LEFT_PORT)), value.clone()),
        (
            FrontValueLocation::Input(port_id(RIGHT_PORT)),
            value.clone(),
        ),
        (FrontValueLocation::Output(port_id(OUT_PORT)), value.clone()),
    ]
    .into_iter()
    .map(|(location, contract)| FrontValueContract { location, contract })
    .collect::<Vec<_>>();
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(FIRST_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_FIRST_CONTRACT_REVISION),
        inputs,
        outputs,
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::Terminal(
                KindTerminalBehavior::FirstReadyLeftTieCancelsLoserOrCompletesWithoutWinner,
            ),
            KindSemanticLaw::ValueContracts(value_contracts),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: FLOW_FIRST_MAXIMUM_PENDING_VALUES,
            max_queue_bytes: value
                .maximum_bytes
                .checked_mul(FLOW_FIRST_MAXIMUM_PENDING_VALUES.into())
                .ok_or("flow/first value envelope overflows its finite queue")?,
        },
    })
}

#[cfg(feature = "form-catalog")]
pub fn install_flow_first_kind(
    value_name: &str,
    value: &CheckedValueContract,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_form::KindSignature {
        kind: FIRST_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(flow_first_contract(value).map_err(str::to_string)?)
        .map_err(|error| alloc::format!("install {FIRST_KIND} for {value_name}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        AbnormalTerminalTransduction, CancellationTransduction, NormalCloseTransduction,
        TerminalTransductionProfile,
    };

    #[test]
    fn specialization_retains_one_exact_kind_and_bound_on_both_inputs_and_winner() {
        let value = CheckedValueContract::new(kind_id("value/text"), 128, Vec::new()).unwrap();
        let contract = flow_first_contract(&value).unwrap();
        assert_eq!(contract.inputs.len(), 2);
        assert_eq!(contract.outputs.len(), 3);
        assert!(contract
            .inputs
            .iter()
            .all(|port| port.value_kind == value.value_kind));
        assert_eq!(contract.outputs[0].value_kind, value.value_kind);
        assert_eq!(
            contract.outputs[1].value_kind.as_str(),
            CANCELLATION_REQUEST_INFO_ID
        );
        assert_eq!(contract.limits.max_queue_items, 2);
        assert_eq!(contract.limits.max_queue_bytes, 256);
        contract.validate().unwrap();
    }

    #[test]
    fn canonical_form_lowers_both_explicit_loser_cancellation_routes() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        for name in [
            FIRST_KIND,
            "test/cancellable-scalar",
            "test/scalar-sink",
            "test/cancel-recovery",
        ] {
            startup
                .insert(conduit_form::KindSignature {
                    kind: name.to_string(),
                    startup_parameters: Vec::new(),
                })
                .unwrap();
        }
        profile
            .insert_kind(flow_first_scalar_semantic_contract())
            .unwrap();

        let scalar_port = |name: &str, direction, temporal| PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
            direction,
            temporal,
            abnormal_kind: None,
        };
        let mut source = Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: kind_id("test/cancellable-scalar"),
            kind_contract_revision: KindIdentity::from("test/cancellable-scalar@1"),
            inputs: vec![
                scalar_port("start", PortDirection::Input, PortTemporal::Value),
                PortDescriptor {
                    port_id: port_id("stop"),
                    value_kind: kind_id(conduit_core::CANCELLATION_REQUEST_INFO_ID),
                    direction: PortDirection::Input,
                    temporal: PortTemporal::Value,
                    abnormal_kind: None,
                },
            ],
            outputs: vec![PortDescriptor {
                abnormal_kind: Some(kind_id("test/cancelled")),
                ..scalar_port(
                    "out",
                    PortDirection::Output,
                    PortTemporal::Flow { closes: true },
                )
            }],
            configuration: Vec::new(),
            semantic_laws: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 2,
                max_queue_items: 1,
                max_queue_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
            },
        };
        source
            .semantic_laws
            .push(KindSemanticLaw::TerminalTransduction(
                TerminalTransductionProfile {
                    input_port_id: port_id("start"),
                    output_port_id: port_id("out"),
                    normal_close: NormalCloseTransduction::NotAccepted,
                    abnormal: AbnormalTerminalTransduction::NotAccepted,
                    cancellation: CancellationTransduction::Request {
                        disposition_kind: kind_id("test/cancelled"),
                    },
                },
            ));
        profile.insert_kind(source).unwrap();
        profile
            .insert_kind(Kind {
                startup_parameters: Vec::new(),
                shorthand: None,
                kind_id: kind_id("test/cancel-recovery"),
                kind_contract_revision: KindIdentity::from("test/cancel-recovery@1"),
                inputs: vec![PortDescriptor {
                    port_id: port_id("terminal"),
                    value_kind: kind_id("test/cancelled"),
                    direction: PortDirection::Input,
                    temporal: PortTemporal::Value,
                    abnormal_kind: Some(kind_id("test/cancelled")),
                }],
                outputs: vec![PortDescriptor {
                    port_id: port_id("recovered"),
                    value_kind: kind_id(conduit_core::UNIT_INFO_ID),
                    direction: PortDirection::Output,
                    temporal: PortTemporal::Value,
                    abnormal_kind: None,
                }],
                configuration: Vec::new(),
                semantic_laws: vec![KindSemanticLaw::TerminalTransduction(
                    TerminalTransductionProfile {
                        input_port_id: port_id("terminal"),
                        output_port_id: port_id("recovered"),
                        normal_close: NormalCloseTransduction::NotAccepted,
                        abnormal: AbnormalTerminalTransduction::Recover,
                        cancellation: CancellationTransduction::NotCancellable,
                    },
                )],
                limits: CapabilityLimits {
                    max_active_instances: 2,
                    max_queue_items: 1,
                    max_queue_bytes: 1,
                },
            })
            .unwrap();
        profile
            .insert_kind(Kind {
                startup_parameters: Vec::new(),
                shorthand: None,
                kind_id: kind_id("test/scalar-sink"),
                kind_contract_revision: KindIdentity::from("test/scalar-sink@1"),
                inputs: vec![scalar_port("in", PortDirection::Input, PortTemporal::Value)],
                outputs: Vec::new(),
                configuration: Vec::new(),
                semantic_laws: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
                },
            })
            .unwrap();

        let form = conduit_form::parse_syntax_document(
            "form race {\n left: test/cancellable-scalar\n right: test/cancellable-scalar\n first: flow/first\n sink: test/scalar-sink\n left-recovery: test/cancel-recovery\n right-recovery: test/cancel-recovery\n left.out >> first.left\n right.out >> first.right\n first.out >> sink.in\n first.cancel-left >> left~\n first.cancel-right >> right~\n left! >> left-recovery.terminal\n right! >> right-recovery.terminal\n}\n",
        );
        let checked = conduit_form::check_syntax_document(&form, &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form(&checked, "race", &profile).unwrap();
        assert_eq!(expanded.connections.len(), 7);
        let cancellation_connections = expanded
            .connections
            .iter()
            .filter(|connection| {
                connection.value_kind.as_str() == conduit_core::CANCELLATION_REQUEST_INFO_ID
            })
            .collect::<Vec<_>>();
        assert_eq!(cancellation_connections.len(), 2);
        assert!(cancellation_connections
            .iter()
            .any(|connection| connection.sink_gear_id.as_str() == "race/left"));
        assert!(cancellation_connections
            .iter()
            .any(|connection| connection.sink_gear_id.as_str() == "race/right"));
    }
}
