//! Hosted std declarations for the reviewed bounded numerical algorithms.
use conduit_core::CapabilityOffer;
pub const NUMERIC_EVALUATION_STD_IMPLEMENTATION: &str = "conduit.std/numeric-evaluation@1";
pub fn numeric_evaluation_std_offers(
    gear: &conduit_plot::CheckedGear,
) -> Result<Vec<CapabilityOffer>, String> {
    let kind = conduit_semantic_catalog::numeric_evaluation_kind(gear)?;
    Ok(conduit_semantic_catalog::NUMERIC_EVALUATION_ALGORITHMS
        .iter()
        .map(|algorithm| {
            conduit_semantic_catalog::numeric_evaluation_offer(
                kind.clone(),
                NUMERIC_EVALUATION_STD_IMPLEMENTATION,
                "conduit.std/prepared-numeric@1",
                *algorithm,
            )
        })
        .collect())
}

pub fn numeric_tensor_std_offers(
    gear: &conduit_plot::CheckedGear,
) -> Result<Vec<CapabilityOffer>, String> {
    let kind = conduit_semantic_catalog::numeric_tensor_kind(gear)?;
    Ok(vec![conduit_semantic_catalog::numeric_tensor_offer(
        kind,
        "conduit.std/numeric-tensor@1",
        "conduit.std/prepared-numeric-tensor@1",
    )])
}
