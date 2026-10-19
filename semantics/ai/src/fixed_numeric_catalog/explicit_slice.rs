//! Exact finite slices with canonical explicit indices, without resource access.
//! Out-of-range starts fail before publication; no hidden clamping or wrapping.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    ExternalEffectBehavior, KindSemanticLaw, ReplayBehavior, SemanticDependence,
    SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
};
pub(crate) fn is_pure_slice(name: &str) -> bool {
    matches!(name, "numeric/slice480x160" | "numeric/slice384x128")
}
pub(super) fn revision(name: &str) -> Option<String> {
    is_pure_slice(name).then(|| {
        format!(
            "{}/{name}/pure-explicit-slice@1",
            super::FIXED_NUMERIC_REVISION
        )
    })
}
pub(super) fn laws(name: &str) -> Vec<KindSemanticLaw> {
    if !is_pure_slice(name) {
        return Vec::new();
    }
    vec![
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
        KindSemanticLaw::TimeDependence(SemanticDependence::None),
        KindSemanticLaw::RandomDependence(SemanticDependence::None),
        KindSemanticLaw::ResourceDependence(SemanticDependence::None),
        KindSemanticLaw::Suspension(SuspensionBehavior::Never),
        KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
        KindSemanticLaw::Replay(ReplayBehavior::Exact),
    ]
}
