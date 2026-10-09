//! Preserve semantic quantity recognition and selected-profile refusals during
//! startup resolution. Numeric eligibility must not fall back to opaque text.

use crate::SyntaxCheckError;
use conduit_core::{Quantity, QuantityLiteralRefusal};

pub(crate) fn startup_quantity(text: &str) -> Result<Option<Quantity>, SyntaxCheckError> {
    match Quantity::parse_plot_literal(text) {
        Ok(value) => Ok(Some(value)),
        Err(QuantityLiteralRefusal::NonCanonicalUnit { canonical }) => {
            Err(SyntaxCheckError::QuantityLiteral(format!(
                "non-canonical quantity unit in '{text}'; use '{canonical}'"
            )))
        }
        Err(
            refusal @ (QuantityLiteralRefusal::RepresentationIneligible { .. }
            | QuantityLiteralRefusal::AmbiguousUnit),
        ) => Err(SyntaxCheckError::QuantityEligibility(
            format!("quantity literal '{text}' refused: {refusal:?}"),
            None,
        )),
        Err(_) => Ok(None),
    }
}
