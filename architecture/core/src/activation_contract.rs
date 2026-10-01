use crate::{
    CheckedValueContract, FlowEachSemanticLaw, FlowFoldAbnormalDisposition,
    FlowFoldCancellationDisposition, FlowFoldCloseDisposition, FlowFoldInvocation,
    FlowFoldSemanticLaw, FlowSelectFalseDisposition, FlowSelectInvocation, FlowSelectRetainedInput,
    FlowSelectSemanticLaw, FlowSelectTrueDisposition, FrontValueContract, FrontValueLocation,
    KindSemanticContract, KindSemanticLaw, PortId, BOOL_INFO_ID,
};
use alloc::{vec, vec::Vec};

fn values(
    input: &CheckedValueContract,
    output: &CheckedValueContract,
    abnormal: Option<&CheckedValueContract>,
    input_port: &PortId,
    output_port: &PortId,
) -> Vec<FrontValueContract> {
    let mut values = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(input_port.clone()),
            contract: input.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(output_port.clone()),
            contract: output.clone(),
        },
    ];
    if let Some(abnormal) = abnormal {
        values.extend([
            FrontValueContract {
                location: FrontValueLocation::InputAbnormal(input_port.clone()),
                contract: abnormal.clone(),
            },
            FrontValueContract {
                location: FrontValueLocation::OutputAbnormal(output_port.clone()),
                contract: abnormal.clone(),
            },
        ]);
    }
    values
}

pub fn flow_each_activation_contract(
    input: &CheckedValueContract,
    output: &CheckedValueContract,
    abnormal: Option<&CheckedValueContract>,
    input_port: PortId,
    output_port: PortId,
    maximum_items: u16,
) -> KindSemanticContract {
    KindSemanticContract {
        configuration: Vec::new(),
        laws: vec![
            KindSemanticLaw::ValueContracts(values(
                input,
                output,
                abnormal,
                &input_port,
                &output_port,
            )),
            KindSemanticLaw::FlowEach(FlowEachSemanticLaw {
                input_port_id: input_port,
                output_port_id: output_port,
                maximum_items,
            }),
        ],
    }
}

pub fn flow_select_activation_contract(
    item: &CheckedValueContract,
    abnormal: Option<&CheckedValueContract>,
    input_port: PortId,
    output_port: PortId,
    maximum_items: u16,
) -> KindSemanticContract {
    KindSemanticContract {
        configuration: Vec::new(),
        laws: vec![
            KindSemanticLaw::ValueContracts(values(
                item,
                item,
                abnormal,
                &input_port,
                &output_port,
            )),
            KindSemanticLaw::FlowSelect(FlowSelectSemanticLaw {
                input_port_id: input_port,
                output_port_id: output_port,
                predicate_input_kind: item.value_kind.clone(),
                predicate_output_kind: crate::kind_id(BOOL_INFO_ID),
                maximum_active: 1,
                maximum_queued: 1,
                maximum_items,
                invocation: FlowSelectInvocation::OncePerAcceptedInput,
                retained_input: FlowSelectRetainedInput::UntilPredicateCompletion,
                true_disposition: FlowSelectTrueDisposition::EmitRetainedInputExactlyOnce,
                false_disposition: FlowSelectFalseDisposition::EmitNothing,
            }),
        ],
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flow_fold_activation_contract(
    item: &CheckedValueContract,
    accumulator: &CheckedValueContract,
    initial_accumulator: Vec<u8>,
    abnormal: Option<&CheckedValueContract>,
    input_port: PortId,
    output_port: PortId,
    combine_accumulator_port_id: PortId,
    combine_item_port_id: PortId,
    combine_output_port_id: PortId,
    maximum_items: u16,
) -> KindSemanticContract {
    KindSemanticContract {
        configuration: Vec::new(),
        laws: vec![
            KindSemanticLaw::ValueContracts(values(
                item,
                accumulator,
                abnormal,
                &input_port,
                &output_port,
            )),
            KindSemanticLaw::FlowFold(FlowFoldSemanticLaw {
                input_port_id: input_port,
                output_port_id: output_port,
                item: item.clone(),
                accumulator: accumulator.clone(),
                initial_accumulator,
                combine_accumulator_port_id,
                combine_item_port_id,
                combine_output_port_id,
                maximum_active: 1,
                maximum_queued: 1,
                maximum_items,
                invocation: FlowFoldInvocation::OncePerAcceptedInput,
                close: FlowFoldCloseDisposition::DrainThenEmitAccumulatorExactlyOnce,
                abnormal: FlowFoldAbnormalDisposition::DiscardAccumulatorAndPropagateExact,
                cancellation: FlowFoldCancellationDisposition::DiscardAccumulatorWithoutEmission,
            }),
        ],
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flow_scan_activation_contract(
    item: &CheckedValueContract,
    accumulator: &CheckedValueContract,
    initial_accumulator: Vec<u8>,
    abnormal: Option<&CheckedValueContract>,
    input_port: PortId,
    output_port: PortId,
    combine_accumulator_port_id: PortId,
    combine_item_port_id: PortId,
    combine_output_port_id: PortId,
    maximum_items: u16,
) -> KindSemanticContract {
    KindSemanticContract {
        configuration: Vec::new(),
        laws: vec![
            KindSemanticLaw::ValueContracts(values(
                item,
                accumulator,
                abnormal,
                &input_port,
                &output_port,
            )),
            KindSemanticLaw::FlowScan(crate::FlowScanSemanticLaw {
                input_port_id: input_port,
                output_port_id: output_port,
                item: item.clone(),
                accumulator: accumulator.clone(),
                initial_accumulator,
                combine_accumulator_port_id,
                combine_item_port_id,
                combine_output_port_id,
                maximum_active: 1,
                maximum_queued: 1,
                maximum_items,
                invocation: crate::FlowScanInvocation::OncePerAcceptedInput,
                progression:
                    crate::FlowScanProgression::EmitCombinedAccumulatorExactlyOnceInInputOrder,
                empty: crate::FlowScanEmptyDisposition::EmitNothing,
                close: crate::FlowScanCloseDisposition::DrainThenCloseWithoutExtraEmission,
                abnormal: crate::FlowScanAbnormalDisposition::DiscardAccumulatorAndPropagateExact,
                cancellation:
                    crate::FlowScanCancellationDisposition::DiscardAccumulatorWithoutEmission,
            }),
        ],
    }
}
