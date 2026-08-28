//! Named proof explanations retained with their exact evidence vocabulary.

#[cfg(test)]
mod heterogeneous_explanation;
#[cfg(test)]
mod heterogeneous_explanation_tests;
mod voyager_explanation;

#[cfg(test)]
pub use heterogeneous_explanation::{
    PatchbayCapstoneBaseline, PatchbayHeterogeneousCapstoneExplanation,
    MAX_CAPSTONE_EXPLANATION_BYTES,
};
pub use voyager_explanation::{
    explain_voyager_capstone, VoyagerCapstoneExplanation, VoyagerCapstoneExplanationError,
    VoyagerScarStageExplanation, MAX_VOYAGER_CAPSTONE_EXPLANATION_BYTES,
};
