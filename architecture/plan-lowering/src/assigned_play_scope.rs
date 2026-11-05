//! Exact execution scope is independent of the assigned Plot's table identity.
use conduit_core as c;

#[derive(Clone, Copy)]
pub enum AssignedPlayScope<'a> {
    Plot(&'a c::ActivePlayIdentity),
    #[cfg(feature = "delegated-preparation")]
    Delegated(&'a conduit_body::DelegatedBodyFragment<'a>),
    /// Constructed by full Body preparation, not from an incoming Plan label.
    #[cfg(feature = "body-plan-binding")]
    Body(crate::body_fragment_scope::PreparedBodyFragment<'a>),
}

impl AssignedPlayScope<'_> {
    pub(crate) fn matches_claim(&self, claim: &c::BaseOperationClaim) -> bool {
        match self {
            Self::Plot(active) => {
                claim.plan_id == active.plan_id
                    && claim.active_play_id == active.active_play_id
                    && claim.host_id == active.host_id
                    && claim.boot_id == active.boot_id
            }
            #[cfg(feature = "delegated-preparation")]
            Self::Delegated(scope) => {
                let m = scope.metadata();
                claim.plan_id.as_str() == m.body_plan
                    && claim.active_play_id.as_str() == m.body_play
                    && claim.host_id.as_str() == m.host
                    && claim.boot_id.as_str() == m.boot
            }
            #[cfg(feature = "body-plan-binding")]
            Self::Body(scope) => {
                claim.plan_id == scope.plan().plan_id
                    && claim.active_play_id == scope.play().active_play_id
                    && claim.host_id == scope.fragment().host_id
                    && claim.boot_id == scope.fragment().boot_id
            }
        }
    }
    pub(crate) fn matches(&self, view: &c::AssignedPlanView, _header: &[u8]) -> bool {
        let id = c::AssignedIdentity::from_text;
        match self {
            Self::Plot(active) => {
                view.plan == id(active.plan_id.as_str())
                    && view.host == id(active.host_id.as_str())
                    && view.boot == id(active.boot_id.as_str())
                    && crate::active_scope::matches(active)
            }
            #[cfg(feature = "delegated-preparation")]
            Self::Delegated(scope) => {
                scope.view() == *view && scope.matches_projection_header(_header)
            }
            #[cfg(feature = "body-plan-binding")]
            Self::Body(scope) => {
                let fragment = scope.fragment();
                view.plan == id(fragment.plan_id.as_str())
                    && view.fragment == id(fragment.fragment_id.as_str())
                    && view.host == id(fragment.host_id.as_str())
                    && view.boot == id(fragment.boot_id.as_str())
            }
        }
    }
}
