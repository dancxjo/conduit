//! Selected legacy storage admission after whole-suffix semantic recognition.
//! Existing legacy spellings retain their original parser and selection order.

use super::{
    ExactDecimalQuantity, ExactDecimalQuantityRefusal, Quantity, QuantityConversionRefusal,
    QuantityLiteralRefusal, QuantityRepresentationRefusal, QUANTITY_INFO_ID,
};

fn ineligible(reason: QuantityRepresentationRefusal) -> QuantityLiteralRefusal {
    QuantityLiteralRefusal::RepresentationIneligible {
        profile: QUANTITY_INFO_ID,
        reason,
    }
}

pub(super) fn project_extended_literal(literal: &str) -> Result<Quantity, QuantityLiteralRefusal> {
    use ExactDecimalQuantityRefusal as Exact;
    use QuantityRepresentationRefusal as Eligibility;
    let exact =
        ExactDecimalQuantity::parse_plot_literal(literal).map_err(|refusal| match refusal {
            Exact::LiteralTooLong => ineligible(Eligibility::LiteralTooLong),
            Exact::NumberTooLong => ineligible(Eligibility::NumberTooLong),
            Exact::SignificantDigitsExceeded => ineligible(Eligibility::SignificantDigitsExceeded),
            Exact::ExponentOutOfRange => ineligible(Eligibility::ExponentOutOfRange),
            Exact::Unit(crate::QuantitySuffixRefusal::Literal(refusal)) => refusal,
            Exact::Unit(crate::QuantitySuffixRefusal::Ambiguous) => {
                QuantityLiteralRefusal::AmbiguousUnit
            }
            _ => QuantityLiteralRefusal::InvalidValue,
        })?;
    for target in super::literal::ALL_QUANTITY_UNITS {
        if target.dimension() != exact.dimension()
            || (exact.dimension() == super::QuantityDimension::Temperature
                && super::literal::temperature_family(exact.unit())
                    != super::literal::temperature_family(target))
        {
            continue;
        }
        match exact.convert_to_legacy(target) {
            Ok(quantity) => return Ok(quantity),
            // An overflow in a fine-grained candidate is not proof that no
            // coarser reviewed legacy unit can represent the same exact value.
            Err(QuantityConversionRefusal::Inexact | QuantityConversionRefusal::Overflow) => {}
            Err(QuantityConversionRefusal::IncompatibleDimensions) => unreachable!(),
        }
    }
    Err(ineligible(Eligibility::NoExactLegacyUnit))
}
