//! Exact generic DSP dispatch through the existing numeric owner factory.
use super::*;
use crate::{
    fixed_numeric_dsp_back::FixedDspBack, fixed_numeric_dsp_catalog::*,
    fixed_numeric_integer_conversion::FixedIntegerConversionBack,
};
pub(super) fn select(
    kind: &str,
    planned: Option<(&PlannedGear, u16)>,
) -> Option<Result<Selection, String>> {
    let (name, flow) = if let Some(name) = kind.strip_prefix("numeric/flow-") {
        (name, true)
    } else {
        (kind.strip_prefix("numeric/")?, false)
    };
    Some(match name {
        "real-dft320" => run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::RealDft320, flow),
            FixedDspBack::<320, 322>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::RealDft320,
                flow
            )
        ),
        "magnitude-squared161" => run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::MagnitudeSquared161, flow),
            FixedDspBack::<322, 161>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::MagnitudeSquared161,
                flow
            )
        ),
        "log10-18" => run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Log10_18, flow),
            FixedDspBack::<18, 18>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Log10_18,
                flow
            )
        ),
        "log10-1" => run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Log10_1, flow),
            FixedDspBack::<1, 1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Log10_1,
                flow
            )
        ),
        "dot160" => run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Dot160, flow),
            FixedDspBack::<160, 1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Dot160,
                flow
            )
        ),
        "sqrt1" => run!(
            planned,
            gear,
            fuel,
            fixed_dsp_offer(FixedDspOperation::Sqrt1, flow),
            FixedDspBack::<1, 1>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                FixedDspOperation::Sqrt1,
                flow
            )
        ),
        "u16-to-f32" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::U16, flow),
                FixedIntegerConversionBack::<1>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::U16, flow)
            )
        }
        "i16-to-f32-80" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::I16Vector80, flow),
                FixedIntegerConversionBack::<80>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::I16Vector80, flow)
            )
        }
        "i16-to-f32-160" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::I16Vector160, flow),
                FixedIntegerConversionBack::<160>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::I16Vector160, flow)
            )
        }
        "finite-vector1" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::FiniteVector(1), flow),
                FixedIntegerConversionBack::<1>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::FiniteVector(1), flow)
            )
        }
        "finite-vector18" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::FiniteVector(18), flow),
                FixedIntegerConversionBack::<18>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::FiniteVector(18), flow)
            )
        }
        "finite-vector20" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::FiniteVector(20), flow),
                FixedIntegerConversionBack::<20>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::FiniteVector(20), flow)
            )
        }
        "finite-vector160" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::FiniteVector(160), flow),
                FixedIntegerConversionBack::<160>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::FiniteVector(160), flow)
            )
        }
        "finite-vector256" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::FiniteVector(256), flow),
                FixedIntegerConversionBack::<256>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::FiniteVector(256), flow)
            )
        }
        "finite-vector320" => {
            run!(
                planned,
                gear,
                fuel,
                fixed_integer_conversion_offer(IntegerConversion::FiniteVector(320), flow),
                FixedIntegerConversionBack::<320>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, IntegerConversion::FiniteVector(320), flow)
            )
        }
        _ => return None,
    })
}
