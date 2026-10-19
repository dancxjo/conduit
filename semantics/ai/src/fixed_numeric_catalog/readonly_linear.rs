//! Reviewed deterministic projection over an explicit immutable tensor input.
//! These facts neither dereference a resource nor admit a compact realization.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    ExternalEffectBehavior, KindSemanticLaw, ReplayBehavior, SemanticDependence,
    SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
};
fn reviewed(name: &str) -> bool {
    matches!(
        name,
        "numeric/linear3x2"
            | "numeric/linear161x18"
            | "numeric/linear18x18"
            | "numeric/linear80x1"
            | "numeric/linear328x192"
            | "numeric/linear192x192"
            | "numeric/linear192x4"
            | "numeric/linear272x480"
            | "numeric/linear160x480"
            | "numeric/linear240x384"
            | "numeric/linear128x384"
            | "numeric/linear208x384"
            | "numeric/linear160x160"
            | "numeric/linear128x128"
            | "numeric/linear688x128"
            | "numeric/linear128x40"
    )
}
pub(super) fn revision(name: &str) -> Option<String> {
    reviewed(name).then(|| {
        format!(
            "{}/{name}/read-immutable-input@1",
            super::FIXED_NUMERIC_REVISION
        )
    })
}
pub(super) fn laws(name: &str) -> Vec<KindSemanticLaw> {
    if !reviewed(name) {
        return Vec::new();
    }
    // The weights input names exact content, shape, resource identity and
    // version. Preparation must separately admit its immutable read grant and
    // validate those bytes. Step still checks that the incoming reference
    // matches the retained resource before arithmetic. No ambient lookup, write,
    // temporal recurrence or Host Call is part of the projection contract.
    vec![
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
        KindSemanticLaw::TimeDependence(SemanticDependence::None),
        KindSemanticLaw::RandomDependence(SemanticDependence::None),
        KindSemanticLaw::ResourceDependence(SemanticDependence::ExplicitInput),
        KindSemanticLaw::Suspension(SuspensionBehavior::Never),
        KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
        KindSemanticLaw::Replay(ReplayBehavior::Exact),
    ]
}
