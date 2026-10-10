//! Exact bounded selection by one reviewed Value-to-Boolean predicate.

#[cfg(feature = "plot-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, FrontValueContract, FrontValueLocation, Kind, KindIdentity,
    KindSemanticLaw, NormalCloseTransduction, PortDescriptor, PortDirection, PortTemporal,
    TerminalTransductionProfile,
};
#[cfg(test)]
use conduit_core::{
    FlowSelectFalseDisposition, FlowSelectInvocation, FlowSelectRetainedInput,
    FlowSelectTrueDisposition, BOOL_INFO_ID,
};

pub const FLOW_SELECT_KIND: &str = "flow/select";
pub const FLOW_SELECT_CONTRACT_REVISION: &str = "conduit.flow/select@1";
pub const FLOW_SELECT_INPUT_PORT: &str = "value";
pub const FLOW_SELECT_OUTPUT_PORT: &str = "selected";

/// Specializes `flow/select` for the exact item accepted by a reviewed
/// Value-to-Boolean predicate. The selected predicate and its realization are
/// checked Plot and Plan truth; this contract owns only coordinator meaning.
pub fn flow_select_semantic_contract(
    item: &CheckedValueContract,
    abnormal: Option<&CheckedValueContract>,
    maximum_items: u16,
) -> Result<Kind, &'static str> {
    if maximum_items == 0 {
        return Err("flow/select maximum-items must be positive");
    }
    require_finite(item, "item")?;
    if let Some(abnormal) = abnormal {
        require_finite(abnormal, "abnormal terminal")?;
    }
    let abnormal_kind = abnormal.map(|value| value.value_kind.clone());
    let input = PortDescriptor {
        port_id: port_id(FLOW_SELECT_INPUT_PORT),
        value_kind: item.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: abnormal_kind.clone(),
    };
    let output = PortDescriptor {
        port_id: port_id(FLOW_SELECT_OUTPUT_PORT),
        value_kind: item.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind,
    };
    let mut contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(input.port_id.clone()),
            contract: item.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(output.port_id.clone()),
            contract: item.clone(),
        },
    ];
    if let Some(abnormal) = abnormal {
        contracts.extend([
            FrontValueContract {
                location: FrontValueLocation::InputAbnormal(input.port_id.clone()),
                contract: abnormal.clone(),
            },
            FrontValueContract {
                location: FrontValueLocation::OutputAbnormal(output.port_id.clone()),
                contract: abnormal.clone(),
            },
        ]);
    }
    let terminal_bytes = abnormal.map_or(0, |value| value.maximum_bytes);
    drop(contracts);
    let mut semantic_laws = conduit_core::flow_select_activation_contract(
        item,
        abnormal,
        input.port_id.clone(),
        output.port_id.clone(),
        maximum_items,
    )
    .laws;
    semantic_laws.push(KindSemanticLaw::TerminalTransduction(
        TerminalTransductionProfile {
            input_port_id: input.port_id.clone(),
            output_port_id: output.port_id.clone(),
            normal_close: NormalCloseTransduction::PropagateAfterDrain,
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
        kind_id: kind_id(FLOW_SELECT_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_SELECT_CONTRACT_REVISION),
        inputs: vec![input],
        outputs: vec![output],
        configuration: Vec::new(),
        semantic_laws,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: item
                .maximum_bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(1))
                .and_then(|bytes| bytes.checked_add(terminal_bytes))
                .ok_or("flow/select finite queue envelope overflows")?,
        },
    })
}

fn require_finite(value: &CheckedValueContract, role: &'static str) -> Result<(), &'static str> {
    if value.maximum_bytes == 0 && value.value_kind.as_str() != conduit_core::EMPTY_INFO_ID {
        return Err(if role == "item" {
            "flow/select requires a finite canonical item envelope"
        } else {
            "flow/select requires a finite canonical abnormal terminal envelope"
        });
    }
    Ok(())
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_select_kind(
    item: &CheckedValueContract,
    abnormal: Option<&CheckedValueContract>,
    maximum_items: u16,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_SELECT_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(
            flow_select_semantic_contract(item, abnormal, maximum_items).map_err(str::to_string)?,
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
    fn contract_retains_exact_item_and_uses_canonical_boolean_predicate() {
        let item = value("value/item", 32);
        let abnormal = value("terminal/predicate", 12);
        let contract = flow_select_semantic_contract(&item, Some(&abnormal), 4).unwrap();
        let law = contract
            .semantic_laws
            .iter()
            .find_map(|law| match law {
                KindSemanticLaw::FlowSelect(law) => Some(law),
                _ => None,
            })
            .unwrap();
        assert_eq!(law.predicate_input_kind, item.value_kind);
        assert_eq!(law.predicate_output_kind.as_str(), BOOL_INFO_ID);
        assert_eq!(law.maximum_active, 1);
        assert_eq!(law.maximum_queued, 1);
        assert_eq!(law.invocation, FlowSelectInvocation::OncePerAcceptedInput);
        assert_eq!(
            law.retained_input,
            FlowSelectRetainedInput::UntilPredicateCompletion
        );
        assert_eq!(
            law.false_disposition,
            FlowSelectFalseDisposition::EmitNothing
        );
        assert_eq!(
            law.true_disposition,
            FlowSelectTrueDisposition::EmitRetainedInputExactlyOnce
        );
        assert_eq!(contract.limits.max_queue_bytes, 77);
        contract.validate().unwrap();
    }

    #[test]
    fn close_and_abnormal_truth_wait_for_admitted_predicate_work() {
        let contract = flow_select_semantic_contract(
            &value("value/item", 8),
            Some(&value("terminal/predicate", 4)),
            4,
        )
        .unwrap();
        let terminal = contract.terminal_transductions().next().unwrap();
        assert_eq!(
            terminal.normal_close,
            NormalCloseTransduction::PropagateAfterDrain
        );
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
    fn nonfinite_envelopes_are_rejected() {
        assert!(flow_select_semantic_contract(&value("value/unbounded", 0), None, 4).is_err());
        assert!(flow_select_semantic_contract(
            &value("value/item", 8),
            Some(&value("terminal/unbounded", 0)),
            4,
        )
        .is_err());
    }
}
