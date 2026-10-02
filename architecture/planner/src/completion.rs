use conduit_core::PlanCompletionPolicy;
use conduit_plot::PlotCompletionPolicy;

pub(crate) const fn plan_completion_policy(policy: PlotCompletionPolicy) -> PlanCompletionPolicy {
    match policy {
        PlotCompletionPolicy::Live => PlanCompletionPolicy::Live,
        PlotCompletionPolicy::SemanticCompletion => PlanCompletionPolicy::SemanticCompletion,
    }
}
