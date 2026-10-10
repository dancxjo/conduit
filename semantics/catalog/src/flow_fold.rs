//! Exact bounded left fold through one reviewed two-input Value combine Plot.

#[cfg(feature = "plot-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
};
#[cfg(test)]
use conduit_core::{
    FlowFoldAbnormalDisposition, FlowFoldCancellationDisposition, FlowFoldCloseDisposition,
    FlowFoldInvocation, KindSemanticLaw,
};

pub const FLOW_FOLD_KIND: &str = "flow/fold";
pub const FLOW_FOLD_CONTRACT_REVISION: &str = "conduit.flow/fold@1";
pub const FLOW_FOLD_INPUT_PORT: &str = "items";
pub const FLOW_FOLD_OUTPUT_PORT: &str = "result";
pub const FLOW_FOLD_COMBINE_ACCUMULATOR_PORT: &str = "accumulator";
pub const FLOW_FOLD_COMBINE_ITEM_PORT: &str = "item";
pub const FLOW_FOLD_COMBINE_OUTPUT_PORT: &str = "combined";

pub fn flow_fold_semantic_contract(
    item: &CheckedValueContract,
    accumulator: &CheckedValueContract,
    initial_accumulator: &[u8],
    abnormal: Option<&CheckedValueContract>,
    maximum_items: u16,
) -> Result<Kind, &'static str> {
    if maximum_items == 0 {
        return Err("flow/fold maximum-items must be positive");
    }
    require_finite(item, "item")?;
    require_finite(accumulator, "accumulator")?;
    if accumulator.validate(initial_accumulator).is_err() {
        return Err("flow/fold initial value is not canonical for its exact accumulator contract");
    }
    if let Some(abnormal) = abnormal {
        require_finite(abnormal, "abnormal terminal")?;
    }
    let abnormal_kind = abnormal.map(|value| value.value_kind.clone());
    let input = PortDescriptor {
        port_id: port_id(FLOW_FOLD_INPUT_PORT),
        value_kind: item.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: abnormal_kind.clone(),
    };
    let output = PortDescriptor {
        port_id: port_id(FLOW_FOLD_OUTPUT_PORT),
        value_kind: accumulator.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind,
    };
    let mut contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(input.port_id.clone()),
            contract: item.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(output.port_id.clone()),
            contract: accumulator.clone(),
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
    let semantic_laws = conduit_core::flow_fold_activation_contract(
        item,
        accumulator,
        initial_accumulator.to_vec(),
        abnormal,
        input.port_id.clone(),
        output.port_id.clone(),
        port_id(FLOW_FOLD_COMBINE_ACCUMULATOR_PORT),
        port_id(FLOW_FOLD_COMBINE_ITEM_PORT),
        port_id(FLOW_FOLD_COMBINE_OUTPUT_PORT),
        maximum_items,
    )
    .laws;
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(FLOW_FOLD_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_FOLD_CONTRACT_REVISION),
        inputs: vec![input],
        outputs: vec![output],
        configuration: Vec::new(),
        semantic_laws,
        limits: CapabilityLimits {
            max_active_instances: 1,
            // retained accumulator, active item, queued item, and final output
            max_queue_items: 4,
            max_queue_bytes: accumulator
                .maximum_bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(item.maximum_bytes.checked_mul(2)?))
                .and_then(|bytes| bytes.checked_add(terminal_bytes))
                .ok_or("flow/fold finite storage envelope overflows")?,
        },
    })
}

fn require_finite(value: &CheckedValueContract, role: &'static str) -> Result<(), &'static str> {
    if value.maximum_bytes == 0 && value.value_kind.as_str() != conduit_core::EMPTY_INFO_ID {
        return Err(match role {
            "item" => "flow/fold requires a finite canonical item envelope",
            "accumulator" => "flow/fold requires a finite canonical accumulator envelope",
            _ => "flow/fold requires a finite canonical abnormal terminal envelope",
        });
    }
    Ok(())
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_fold_kind(
    item: &CheckedValueContract,
    accumulator: &CheckedValueContract,
    initial_accumulator: &[u8],
    abnormal: Option<&CheckedValueContract>,
    maximum_items: u16,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_FOLD_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(
            flow_fold_semantic_contract(
                item,
                accumulator,
                initial_accumulator,
                abnormal,
                maximum_items,
            )
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
    fn exact_fold_law_seals_initial_state_and_combine_fronts() {
        let item = value("value/u32", 4);
        let accumulator = value("value/u64", 8);
        let initial = 7_u64.to_le_bytes();
        let kind = flow_fold_semantic_contract(&item, &accumulator, &initial, None, 4).unwrap();
        let KindSemanticLaw::FlowFold(law) = &kind.semantic_laws[1] else {
            panic!()
        };
        assert_eq!(law.initial_accumulator, initial);
        assert_eq!(law.combine_accumulator_port_id.as_str(), "accumulator");
        assert_eq!(law.combine_item_port_id.as_str(), "item");
        assert_eq!(law.combine_output_port_id.as_str(), "combined");
        assert_eq!(law.maximum_active, 1);
        assert_eq!(law.maximum_queued, 1);
        assert_eq!(kind.outputs[0].temporal, PortTemporal::Value);
        kind.validate().unwrap();
    }

    #[test]
    fn empty_input_and_drain_meaning_is_explicit() {
        let initial = 9_u64.to_le_bytes();
        let kind = flow_fold_semantic_contract(
            &value("value/u32", 4),
            &value("value/u64", 8),
            &initial,
            Some(&value("terminal/fold", 2)),
            4,
        )
        .unwrap();
        let KindSemanticLaw::FlowFold(law) = &kind.semantic_laws[1] else {
            panic!()
        };
        assert_eq!(
            law.close,
            FlowFoldCloseDisposition::DrainThenEmitAccumulatorExactlyOnce
        );
        assert_eq!(law.invocation, FlowFoldInvocation::OncePerAcceptedInput);
        assert_eq!(
            law.abnormal,
            FlowFoldAbnormalDisposition::DiscardAccumulatorAndPropagateExact
        );
        assert_eq!(
            law.cancellation,
            FlowFoldCancellationDisposition::DiscardAccumulatorWithoutEmission
        );
        assert_eq!(kind.limits.max_queue_items, 4);
        assert_eq!(kind.limits.max_queue_bytes, 26);
    }

    #[test]
    fn refuses_inexact_initial_and_unbounded_storage() {
        let item = value("value/u32", 4);
        let accumulator = value("value/u64", 8);
        assert!(flow_fold_semantic_contract(&item, &accumulator, &[0; 7], None, 4).is_err());
        assert!(flow_fold_semantic_contract(
            &value("value/unbounded", 0),
            &accumulator,
            &[0; 8],
            None,
            4,
        )
        .is_err());
    }

    #[test]
    fn validation_refuses_fold_law_drift() {
        let mut kind = flow_fold_semantic_contract(
            &value("value/u32", 4),
            &value("value/u64", 8),
            &0_u64.to_le_bytes(),
            None,
            4,
        )
        .unwrap();
        let KindSemanticLaw::FlowFold(law) = &mut kind.semantic_laws[1] else {
            panic!()
        };
        law.maximum_active = 2;
        assert!(kind.validate().is_err());

        let mut kind = flow_fold_semantic_contract(
            &value("value/u32", 4),
            &value("value/u64", 8),
            &0_u64.to_le_bytes(),
            None,
            4,
        )
        .unwrap();
        let KindSemanticLaw::FlowFold(law) = &mut kind.semantic_laws[1] else {
            panic!()
        };
        law.initial_accumulator.pop();
        assert!(kind.validate().is_err());
    }
}
