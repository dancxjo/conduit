use super::*;
use conduit_ai::fixed_numeric_integer_conversion::FixedIntegerConversionBack;
use conduit_ai::{fixed_numeric_dsp_back::FixedDspBack, fixed_numeric_dsp_catalog::*};
use conduit_ai::{
    fixed_numeric_index_back::*, fixed_numeric_pair_back::*,
    fixed_numeric_preparation::fixed_window_offer, fixed_numeric_scan_back::*,
    fixed_numeric_signal_back::*, fixed_numeric_window_back::*,
};
pub(super) fn select(
    kind: &str,
    planned: Option<(&PlannedGear, u16)>,
) -> Option<Result<Selection, String>> {
    match kind {
        "numeric/u16-to-f32" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::U16, false),
                FixedIntegerConversionBack::<1>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::U16, false)
            ))
        }
        "numeric/i16-to-f32-160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::I16Vector160, false),
                FixedIntegerConversionBack::<160>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::I16Vector160, false)
            ))
        }
        "numeric/real-dft320" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::RealDft320, false),
            FixedDspBack::<320, 322>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::RealDft320,
                false
            )
        )),
        "numeric/magnitude-squared161" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::MagnitudeSquared161, false),
            FixedDspBack::<322, 161>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::MagnitudeSquared161,
                false
            )
        )),
        "numeric/log10-18" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Log10_18, false),
            FixedDspBack::<18, 18>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Log10_18,
                false
            )
        )),
        "numeric/log10-1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Log10_1, false),
            FixedDspBack::<1, 1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Log10_1,
                false
            )
        )),
        "numeric/dot160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Dot160, false),
            FixedDspBack::<160, 1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Dot160,
                false
            )
        )),
        "numeric/sqrt1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Sqrt1, false),
            FixedDspBack::<1, 1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Sqrt1,
                false
            )
        )),
        "numeric/multiply320" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<320>(FixedElementwiseOperation::Multiply),
            FixedElementwiseBack::<320>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Multiply
            )
        )),
        "numeric/add40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<40>(FixedElementwiseOperation::Add),
            FixedElementwiseBack::<40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Add
            )
        )),
        "numeric/add128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<128>(FixedElementwiseOperation::Add),
            FixedElementwiseBack::<128>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Add
            )
        )),
        "numeric/add160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<160>(FixedElementwiseOperation::Add),
            FixedElementwiseBack::<160>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Add
            )
        )),
        "numeric/add192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<192>(FixedElementwiseOperation::Add),
            FixedElementwiseBack::<192>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Add
            )
        )),
        "numeric/multiply40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<40>(FixedElementwiseOperation::Multiply),
            FixedElementwiseBack::<40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Multiply
            )
        )),
        "numeric/multiply128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<128>(FixedElementwiseOperation::Multiply),
            FixedElementwiseBack::<128>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Multiply
            )
        )),
        "numeric/multiply160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<160>(FixedElementwiseOperation::Multiply),
            FixedElementwiseBack::<160>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Multiply
            )
        )),
        "numeric/multiply192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<192>(FixedElementwiseOperation::Multiply),
            FixedElementwiseBack::<192>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Multiply
            )
        )),
        "numeric/sigmoid4" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<4>(FixedElementwiseOperation::Sigmoid),
            FixedElementwiseBack::<4>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Sigmoid
            )
        )),
        "numeric/sigmoid40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<40>(FixedElementwiseOperation::Sigmoid),
            FixedElementwiseBack::<40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Sigmoid
            )
        )),
        "numeric/sigmoid128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<128>(FixedElementwiseOperation::Sigmoid),
            FixedElementwiseBack::<128>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Sigmoid
            )
        )),
        "numeric/sigmoid160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<160>(FixedElementwiseOperation::Sigmoid),
            FixedElementwiseBack::<160>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Sigmoid
            )
        )),
        "numeric/sigmoid192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<192>(FixedElementwiseOperation::Sigmoid),
            FixedElementwiseBack::<192>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Sigmoid
            )
        )),
        "numeric/complement40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<40>(FixedElementwiseOperation::Complement),
            FixedElementwiseBack::<40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Complement
            )
        )),
        "numeric/complement128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<128>(FixedElementwiseOperation::Complement),
            FixedElementwiseBack::<128>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Complement
            )
        )),
        "numeric/complement160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<160>(FixedElementwiseOperation::Complement),
            FixedElementwiseBack::<160>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Complement
            )
        )),
        "numeric/complement192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<192>(FixedElementwiseOperation::Complement),
            FixedElementwiseBack::<192>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Complement
            )
        )),
        "numeric/exp1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<1>(FixedElementwiseOperation::Exp),
            FixedElementwiseBack::<1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Exp
            )
        )),
        "numeric/scale40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<40>(FixedElementwiseOperation::Scale),
            FixedElementwiseBack::<40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Scale
            )
        )),
        "numeric/scale44" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<44>(FixedElementwiseOperation::Scale),
            FixedElementwiseBack::<44>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Scale
            )
        )),
        "numeric/clamp40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<40>(FixedElementwiseOperation::Clamp),
            FixedElementwiseBack::<40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Clamp
            )
        )),
        "numeric/clamp44" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<44>(FixedElementwiseOperation::Clamp),
            FixedElementwiseBack::<44>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::Clamp
            )
        )),
        "numeric/reciprocal-offset1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_elementwise_offer::<1>(FixedElementwiseOperation::ReciprocalOffset),
            FixedElementwiseBack::<1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedElementwiseOperation::ReciprocalOffset
            )
        )),
        "numeric/slice320x80" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<320, 80>(FixedIndexOperation::Slice),
            FixedIndexBack::<320, 80>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/slice480x160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<480, 160>(FixedIndexOperation::Slice),
            FixedIndexBack::<480, 160>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/slice384x128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<384, 128>(FixedIndexOperation::Slice),
            FixedIndexBack::<384, 128>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/slice44x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<44, 40>(FixedIndexOperation::Slice),
            FixedIndexBack::<44, 40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/slice256x216" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<256, 216>(FixedIndexOperation::Slice),
            FixedIndexBack::<256, 216>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/slice256x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<256, 40>(FixedIndexOperation::Slice),
            FixedIndexBack::<256, 40>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/slice4x1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<4, 1>(FixedIndexOperation::Slice),
            FixedIndexBack::<4, 1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Slice
            )
        )),
        "numeric/gather256x44" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_index_offer::<256, 44>(FixedIndexOperation::Gather),
            FixedIndexBack::<256, 44>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedIndexOperation::Gather
            )
        )),
        "numeric/one-pole40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_one_pole_offer(),
            FixedOnePoleBack::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
        )),
        "numeric/history2x64" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_window_offer(),
            FixedWindowBack::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
        )),
        name if name.starts_with("numeric/pair") => Some(run!(
            planned,
            gear,
            fuel,
            fixed_value_pair_offer(name),
            FixedValuePairBack::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
        )),
        _ => None,
    }
}
