use super::*;
use conduit_ai::fixed_numeric_operations_back::*;
pub(super) fn select(
    kind: &str,
    planned: Option<(&PlannedGear, u16)>,
) -> Option<Result<Selection, String>> {
    match kind {
        "numeric/concatenate18x1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<18, 1>(),
            FixedConcatenateBack::<18, 1, 19>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/concatenate19x1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<19, 1>(),
            FixedConcatenateBack::<19, 1, 20>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/tanh4" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_tanh_offer::<4>(),
            FixedTanhBack::<4>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
        )),
        "numeric/tanh40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_tanh_offer::<40>(),
            FixedTanhBack::<40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
        )),
        "numeric/tanh64" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_tanh_offer::<64>(),
            FixedTanhBack::<64>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
        )),
        "numeric/tanh128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_tanh_offer::<128>(),
            FixedTanhBack::<128>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/tanh160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_tanh_offer::<160>(),
            FixedTanhBack::<160>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/tanh192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_tanh_offer::<192>(),
            FixedTanhBack::<192>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/tanh320" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_tanh_offer::<320>(),
            FixedTanhBack::<320>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear, fuel
            )
        )),
        "numeric/concatenate20x12" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<20, 12>(),
            FixedConcatenateBack::<20, 12, 32>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate80x44" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<80, 44>(),
            FixedConcatenateBack::<80, 44, 124>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate124x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<124, 40>(),
            FixedConcatenateBack::<124, 40, 164>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate164x164" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<164, 164>(),
            FixedConcatenateBack::<164, 164, 328>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate192x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<192, 40>(),
            FixedConcatenateBack::<192, 40, 232>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate232x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<232, 40>(),
            FixedConcatenateBack::<232, 40, 272>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate160x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<160, 40>(),
            FixedConcatenateBack::<160, 40, 200>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate200x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<200, 40>(),
            FixedConcatenateBack::<200, 40, 240>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate128x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<128, 40>(),
            FixedConcatenateBack::<128, 40, 168>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate168x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<168, 40>(),
            FixedConcatenateBack::<168, 40, 208>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate160x128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<160, 128>(),
            FixedConcatenateBack::<160, 128, 288>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate288x128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<288, 128>(),
            FixedConcatenateBack::<288, 128, 416>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate416x192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<416, 192>(),
            FixedConcatenateBack::<416, 192, 608>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate608x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<608, 40>(),
            FixedConcatenateBack::<608, 40, 648>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate648x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<648, 40>(),
            FixedConcatenateBack::<648, 40, 688>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        "numeric/concatenate216x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_concatenate_offer::<216, 40>(),
            FixedConcatenateBack::<216, 40, 256>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel)
        )),
        _ => None,
    }
}
