//! Preparation storage from the complete canonical Type domain, capped by the
//! existing admitted channel envelope. This reserves no provider or authority.
use conduit_core::{StructuredInfoRefusal as R, StructuredInfoType};

pub(crate) fn bytes(ty: &StructuredInfoType, ceiling: usize) -> Result<usize, R> {
    let bound = conduit_plot::maximum_prepared_canonical_value_bytes(ty)
        .map_err(|_| R::CanonicalEncodingTooLarge)?;
    let ceiling = u32::try_from(ceiling).map_err(|_| R::CanonicalEncodingTooLarge)?;
    // Minimise before converting to the target's usize, including small MCUs.
    Ok(bound.min(ceiling) as usize)
}
