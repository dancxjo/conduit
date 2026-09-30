//! Exact bounded temporal lifting of one reviewed Value transform over a closing Flow.

#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, FiniteTerminalEmission, FrontValueContract, FrontValueLocation, Kind,
    KindIdentity, KindSemanticLaw, NormalCloseTransduction, PortDescriptor, PortDirection,
    PortTemporal, TerminalTransductionProfile,
};

pub const FLOW_EACH_KIND: &str = "flow/each";
pub const FLOW_EACH_CONTRACT_REVISION: &str = "conduit.flow/each@1";
pub const FLOW_EACH_INPUT_PORT: &str = "value";
pub const FLOW_EACH_OUTPUT_PORT: &str = "mapped";

/// Specializes `flow/each` over the exact checked fronts of one reviewed
/// Value-to-Value transform.
///
/// The selected transform and its realization belong to checked Form and Plan
/// truth. This Kind records only the portable coordinator law: one input may be
/// pending while one activation is active, every accepted value owes at most
/// one output, and terminal truth propagates only after that finite work drains.
pub fn flow_each_semantic_contract(
    input: &CheckedValueContract,
    output: &CheckedValueContract,
    abnormal: Option<&CheckedValueContract>,
    maximum_items: u16,
) -> Result<Kind, &'static str> {
    if maximum_items == 0 {
        return Err("flow/each maximum-items must be positive");
    }
    require_finite_envelope(input, "input")?;
    require_finite_envelope(output, "output")?;
    if let Some(abnormal) = abnormal {
        require_finite_envelope(abnormal, "abnormal terminal")?;
    }

    let abnormal_kind = abnormal.map(|contract| contract.value_kind.clone());
    let input_port = PortDescriptor {
        port_id: port_id(FLOW_EACH_INPUT_PORT),
        value_kind: input.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: abnormal_kind.clone(),
    };
    let output_port = PortDescriptor {
        port_id: port_id(FLOW_EACH_OUTPUT_PORT),
        value_kind: output.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind,
    };
    let mut value_contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(input_port.port_id.clone()),
            contract: input.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(output_port.port_id.clone()),
            contract: output.clone(),
        },
    ];
    if let Some(abnormal) = abnormal {
        value_contracts.extend([
            FrontValueContract {
                location: FrontValueLocation::InputAbnormal(input_port.port_id.clone()),
                contract: abnormal.clone(),
            },
            FrontValueContract {
                location: FrontValueLocation::OutputAbnormal(output_port.port_id.clone()),
                contract: abnormal.clone(),
            },
        ]);
    }

    let terminal_bytes = abnormal.map_or(0, |contract| contract.maximum_bytes);
    drop(value_contracts);
    let mut semantic_laws = conduit_core::flow_each_activation_contract(
        input,
        output,
        abnormal,
        input_port.port_id.clone(),
        output_port.port_id.clone(),
        maximum_items,
    )
    .laws;
    semantic_laws.push(KindSemanticLaw::TerminalTransduction(
        TerminalTransductionProfile {
            input_port_id: port_id(FLOW_EACH_INPUT_PORT),
            output_port_id: port_id(FLOW_EACH_OUTPUT_PORT),
            normal_close: NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: output.maximum_bytes,
            }),
            abnormal: if abnormal.is_some() {
                AbnormalTerminalTransduction::PropagateAfterDrain
            } else {
                AbnormalTerminalTransduction::NotAccepted
            },
            cancellation: CancellationTransduction::NotCancellable,
        },
    ));
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(FLOW_EACH_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_EACH_CONTRACT_REVISION),
        inputs: vec![input_port],
        outputs: vec![output_port],
        configuration: Vec::new(),
        semantic_laws,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: input
                .maximum_bytes
                .checked_add(output.maximum_bytes)
                .and_then(|bytes| bytes.checked_add(terminal_bytes))
                .ok_or("flow/each finite queue envelope overflows")?,
        },
    })
}

fn require_finite_envelope(
    contract: &CheckedValueContract,
    role: &'static str,
) -> Result<(), &'static str> {
    if contract.maximum_bytes == 0 && contract.value_kind.as_str() != conduit_core::UNIT_INFO_ID {
        return Err(match role {
            "input" => "flow/each requires a finite canonical input envelope",
            "output" => "flow/each requires a finite canonical output envelope",
            _ => "flow/each requires a finite canonical abnormal terminal envelope",
        });
    }
    Ok(())
}

#[cfg(feature = "form-catalog")]
pub fn install_flow_each_kind(
    input: &CheckedValueContract,
    output: &CheckedValueContract,
    abnormal: Option<&CheckedValueContract>,
    maximum_items: u16,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_form::KindSignature {
        kind: FLOW_EACH_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(
            flow_each_semantic_contract(input, output, abnormal, maximum_items)
                .map_err(str::to_string)?,
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(kind: &str, bytes: u32) -> CheckedValueContract {
        CheckedValueContract::new(kind_id(kind), bytes, vec![]).unwrap()
    }

    #[test]
    fn specialization_preserves_exact_closing_fronts_and_abnormal_kind() {
        let input = value("value/source", 32);
        let output = value("value/mapped", 48);
        let abnormal = value("terminal/transform", 12);
        let contract = flow_each_semantic_contract(&input, &output, Some(&abnormal), 4).unwrap();

        assert_eq!(contract.inputs.len(), 1);
        assert_eq!(contract.outputs.len(), 1);
        assert_eq!(contract.inputs[0].port_id, port_id("value"));
        assert_eq!(contract.outputs[0].port_id, port_id("mapped"));
        assert_eq!(contract.inputs[0].value_kind, input.value_kind);
        assert_eq!(contract.outputs[0].value_kind, output.value_kind);
        assert_eq!(
            contract.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(
            contract.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(
            contract.inputs[0].abnormal_kind,
            Some(abnormal.value_kind.clone())
        );
        assert_eq!(contract.outputs[0].abnormal_kind, Some(abnormal.value_kind));
        contract.validate().unwrap();
    }

    #[test]
    fn specialization_states_finite_pressure_and_terminal_laws() {
        let contract = flow_each_semantic_contract(
            &value("value/source", 32),
            &value("value/mapped", 48),
            Some(&value("terminal/transform", 12)),
            4,
        )
        .unwrap();

        assert_eq!(contract.limits.max_active_instances, 1);
        assert_eq!(contract.limits.max_queue_items, 3);
        assert_eq!(contract.limits.max_queue_bytes, 92);
        let terminal = contract.terminal_transductions().next().unwrap();
        assert!(matches!(
            terminal.normal_close,
            NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: 48,
            })
        ));
        assert_eq!(
            terminal.abnormal,
            AbnormalTerminalTransduction::PropagateAfterDrain
        );
        assert_eq!(
            terminal.cancellation,
            CancellationTransduction::NotCancellable
        );
    }

    #[test]
    fn specialization_rejects_nonfinite_front_envelopes() {
        let unbounded = value("value/unbounded", 0);
        let finite = value("value/finite", 8);
        assert!(flow_each_semantic_contract(&unbounded, &finite, None, 4).is_err());
        assert!(flow_each_semantic_contract(&finite, &unbounded, None, 4).is_err());
        assert!(flow_each_semantic_contract(&finite, &finite, Some(&unbounded), 4).is_err());
    }

    #[test]
    fn specialization_without_abnormal_fronts_rejects_abnormal_terminal_truth() {
        let contract = flow_each_semantic_contract(
            &value("value/source", 32),
            &value("value/mapped", 48),
            None,
            4,
        )
        .unwrap();

        assert_eq!(contract.inputs[0].abnormal_kind, None);
        assert_eq!(contract.outputs[0].abnormal_kind, None);
        assert_eq!(
            contract.terminal_transductions().next().unwrap().abnormal,
            AbnormalTerminalTransduction::NotAccepted
        );
        contract.validate().unwrap();
    }
}
