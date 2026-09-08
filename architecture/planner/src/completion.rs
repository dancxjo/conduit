use conduit_core::PlanCompletionPolicy;
use conduit_form::FormCompletionPolicy;

pub(crate) const fn plan_completion_policy(policy: FormCompletionPolicy) -> PlanCompletionPolicy {
    match policy {
        FormCompletionPolicy::Live => PlanCompletionPolicy::Live,
        FormCompletionPolicy::SemanticCompletion => PlanCompletionPolicy::SemanticCompletion,
    }
}
