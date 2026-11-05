//! Preparation-only containment evidence, never an execution or resource grant.
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::PlanFragment;

/// Borrows immutable checked context. Constructing this witness may allocate
/// while validating the rich Body Plan; using it neither clones nor replans it.
/// This module is absent from default constrained-Host builds.
#[derive(Clone, Copy)]
pub struct PreparedBodyFragment<'a> {
    plan: &'a BodyPlan,
    play: &'a BodyPlayIdentity,
    fragment: &'a PlanFragment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyFragmentScopeRefusal;

impl<'a> PreparedBodyFragment<'a> {
    pub fn prepare(
        wake: &Wake,
        plan: &'a BodyPlan,
        play: &'a BodyPlayIdentity,
        fragment: &'a PlanFragment,
    ) -> Result<Self, BodyFragmentScopeRefusal> {
        if plan.validate_for(wake).is_err()
            || !play.validate_for(plan)
            || plan
                .plots
                .iter()
                .flat_map(|p| &p.plan.fragments)
                .filter(|f| *f == fragment)
                .count()
                != 1
        {
            return Err(BodyFragmentScopeRefusal);
        }
        Ok(Self {
            plan,
            play,
            fragment,
        })
    }
    pub fn plan(&self) -> &'a BodyPlan {
        self.plan
    }
    pub fn play(&self) -> &'a BodyPlayIdentity {
        self.play
    }
    pub fn fragment(&self) -> &'a PlanFragment {
        self.fragment
    }
}
