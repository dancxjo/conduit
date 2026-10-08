use super::*;
use crate::{
    fixed_numeric_index_back::*, fixed_numeric_operations_back::*, fixed_numeric_signal_back::*,
    fixed_numeric_temporal::*,
};
use crate::{fixed_numeric_scan_back::*, fixed_numeric_window_back::*};
pub(super) fn select(
    kind: &str,
    planned: Option<(&PlannedGear, u16)>,
) -> Option<Result<Selection, String>> {
    match kind {
        "numeric/flow-history2x64" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/history2x64", FLOW_WINDOW_IMPLEMENTATION),
            FixedWindowBack::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-one-pole40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/one-pole40", FLOW_SCAN_IMPLEMENTATION),
            FixedOnePoleBack::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-concatenate1x1" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate1x1", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<1, 1, 2>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate2x1" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate2x1", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<2, 1, 3>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-gather640x160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/gather640x160", FLOW_INDEX_IMPLEMENTATION),
                FixedIndexBack::<640, 160>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Gather)
            ))
        }
        "numeric/flow-concatenate18x1" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate18x1", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<18, 1, 19>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate19x1" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate19x1", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<19, 1, 20>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-tanh4" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/tanh4", FLOW_OPERATION_IMPLEMENTATION),
            FixedTanhBack::<4>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-tanh40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/tanh40", FLOW_OPERATION_IMPLEMENTATION),
            FixedTanhBack::<40>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-tanh64" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/tanh64", FLOW_OPERATION_IMPLEMENTATION),
            FixedTanhBack::<64>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-tanh128" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/tanh128", FLOW_OPERATION_IMPLEMENTATION),
            FixedTanhBack::<128>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-tanh160" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/tanh160", FLOW_OPERATION_IMPLEMENTATION),
            FixedTanhBack::<160>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-tanh192" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/tanh192", FLOW_OPERATION_IMPLEMENTATION),
            FixedTanhBack::<192>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-tanh320" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/tanh320", FLOW_OPERATION_IMPLEMENTATION),
            FixedTanhBack::<320>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/flow-concatenate20x12" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate20x12", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<20, 12, 32>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate80x44" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate80x44", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<80, 44, 124>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate124x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate124x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<124, 40, 164>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate164x164" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate164x164", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<164, 164, 328>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate192x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate192x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<192, 40, 232>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate232x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate232x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<232, 40, 272>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate160x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate160x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<160, 40, 200>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate200x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate200x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<200, 40, 240>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate128x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate128x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<128, 40, 168>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate168x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate168x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<168, 40, 208>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate160x128" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate160x128", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<160, 128, 288>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate288x128" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate288x128", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<288, 128, 416>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate416x192" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate416x192", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<416, 192, 608>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate608x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate608x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<608, 40, 648>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate648x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate648x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<648, 40, 688>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-concatenate216x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/concatenate216x40", FLOW_OPERATION_IMPLEMENTATION),
            FixedConcatenateBack::<216, 40, 256>::prepare_flow_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/flow-multiply320" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/multiply320", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<320>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Multiply)
            ))
        }
        "numeric/flow-add40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/add40", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<40>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Add)
            ))
        }
        "numeric/flow-add128" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/add128", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<128>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Add)
            ))
        }
        "numeric/flow-add160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/add160", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<160>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Add)
            ))
        }
        "numeric/flow-add192" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/add192", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<192>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Add)
            ))
        }
        "numeric/flow-multiply40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/multiply40", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<40>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Multiply)
            ))
        }
        "numeric/flow-multiply128" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/multiply128", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<128>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Multiply)
            ))
        }
        "numeric/flow-multiply160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/multiply160", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<160>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Multiply)
            ))
        }
        "numeric/flow-multiply192" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/multiply192", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<192>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Multiply)
            ))
        }
        "numeric/flow-sigmoid4" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/sigmoid4", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<4>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Sigmoid)
            ))
        }
        "numeric/flow-sigmoid40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/sigmoid40", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<40>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Sigmoid)
            ))
        }
        "numeric/flow-sigmoid128" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/sigmoid128", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<128>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Sigmoid)
            ))
        }
        "numeric/flow-sigmoid160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/sigmoid160", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<160>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Sigmoid)
            ))
        }
        "numeric/flow-sigmoid192" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/sigmoid192", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<192>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Sigmoid)
            ))
        }
        "numeric/flow-complement40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/complement40", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<40>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Complement)
            ))
        }
        "numeric/flow-complement128" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/complement128", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<128>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Complement)
            ))
        }
        "numeric/flow-complement160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/complement160", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<160>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Complement)
            ))
        }
        "numeric/flow-complement192" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/complement192", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<192>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Complement)
            ))
        }
        "numeric/flow-exp1" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/exp1", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<1>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Exp)
            ))
        }
        "numeric/flow-scale40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/scale40", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<40>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Scale)
            ))
        }
        "numeric/flow-scale44" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/scale44", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<44>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Scale)
            ))
        }
        "numeric/flow-clamp40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/clamp40", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<40>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Clamp)
            ))
        }
        "numeric/flow-clamp44" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/clamp44", FLOW_ELEMENTWISE_IMPLEMENTATION),
                FixedElementwiseBack::<44>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::Clamp)
            ))
        }
        "numeric/flow-reciprocal-offset1" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer(
                    "numeric/reciprocal-offset1",
                    FLOW_ELEMENTWISE_IMPLEMENTATION
                ),
                FixedElementwiseBack::<1>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedElementwiseOperation::ReciprocalOffset)
            ))
        }
        "numeric/flow-slice320x80" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/slice320x80", FLOW_INDEX_IMPLEMENTATION),
                FixedIndexBack::<320, 80>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Slice)
            ))
        }
        "numeric/flow-slice480x160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/slice480x160", FLOW_INDEX_IMPLEMENTATION),
                FixedIndexBack::<480, 160>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Slice)
            ))
        }
        "numeric/flow-slice384x128" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/slice384x128", FLOW_INDEX_IMPLEMENTATION),
                FixedIndexBack::<384, 128>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Slice)
            ))
        }
        "numeric/flow-slice44x40" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/slice44x40", FLOW_INDEX_IMPLEMENTATION),
            FixedIndexBack::<44, 40>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/flow-slice256x216" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/slice256x216", FLOW_INDEX_IMPLEMENTATION),
                FixedIndexBack::<256, 216>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Slice)
            ))
        }
        "numeric/flow-slice256x40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/slice256x40", FLOW_INDEX_IMPLEMENTATION),
                FixedIndexBack::<256, 40>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Slice)
            ))
        }
        "numeric/flow-slice4x1" => Some(run!(
            planned,
            gear,
            fuel,
            closing_numeric_offer("numeric/slice4x1", FLOW_INDEX_IMPLEMENTATION),
            FixedIndexBack::<4, 1>::prepare_flow_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/flow-gather256x44" => {
            Some(run!(
                planned,
                gear,
                fuel,
                closing_numeric_offer("numeric/gather256x44", FLOW_INDEX_IMPLEMENTATION),
                FixedIndexBack::<256, 44>::prepare_flow_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Gather)
            ))
        }
        _ => None,
    }
}
