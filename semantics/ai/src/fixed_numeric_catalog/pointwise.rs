//! Owned semantic facts for finite pointwise Value transforms only.
//! Tensor operations, temporal operators and resource/index operations are not
//! covered by these declarations, even when their selected Back has no effects.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    ExternalEffectBehavior, KindSemanticLaw, ReplayBehavior, SemanticDependence,
    SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
};
pub(crate) fn is_pure_pointwise(name: &str) -> bool {
    matches!(
        name,
        "numeric/add40"
            | "numeric/multiply40"
            | "numeric/sigmoid40"
            | "numeric/complement40"
            | "numeric/add128"
            | "numeric/multiply128"
            | "numeric/sigmoid128"
            | "numeric/complement128"
            | "numeric/add160"
            | "numeric/multiply160"
            | "numeric/sigmoid160"
            | "numeric/complement160"
            | "numeric/add192"
            | "numeric/multiply192"
            | "numeric/sigmoid192"
            | "numeric/complement192"
            | "numeric/tanh4"
            | "numeric/tanh40"
            | "numeric/tanh64"
            | "numeric/tanh128"
            | "numeric/tanh160"
            | "numeric/tanh192"
            | "numeric/tanh320"
            | "numeric/sigmoid4"
            | "numeric/multiply320"
    )
}
pub(super) fn revision(name: &str) -> String {
    let base = format!("{}/{name}", super::FIXED_NUMERIC_REVISION);
    if is_pure_pointwise(name) {
        format!("{base}/pure-value@1")
    } else {
        base
    }
}
pub(super) fn laws(name: &str) -> Vec<KindSemanticLaw> {
    if !is_pure_pointwise(name) {
        return Vec::new();
    }
    // The finite input/output and libm arithmetic contracts are unchanged.
    // These operators have no semantic retained state; their Back's publication
    // marker is invocation bookkeeping rather than a temporal recurrence.
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
