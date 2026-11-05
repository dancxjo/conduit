//! Preparation-only containment evidence, never an execution or resource grant.
use crate::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::PlanFragment;

/// Borrows immutable checked context. Constructing this witness may allocate
/// while validating the rich Body Plan; using it neither clones nor replans it.
/// A constrained receiver uses the separately validated compact projection; it
/// need not invoke this rich preparation path.
#[derive(Clone, Copy)]
pub struct PreparedBodyFragment<'a> {
    plan: &'a BodyPlan,
    play: &'a BodyPlayIdentity,
    fragment: &'a PlanFragment,
    plot_plan: &'a conduit_core::Plan,
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
        if plan.validate_for(wake).is_err() || !play.validate_for(plan) {
            return Err(BodyFragmentScopeRefusal);
        }
        let plans = plan.plots.iter().map(|p| &p.plan).chain(
            plan.mask_topologies
                .iter()
                .flat_map(|t| t.chains.iter().map(|c| &c.plan)),
        );
        let mut found = None;
        for candidate in plans {
            for f in &candidate.fragments {
                if f == fragment && found.replace(candidate).is_some() {
                    return Err(BodyFragmentScopeRefusal);
                }
            }
        }
        let plot_plan = found.ok_or(BodyFragmentScopeRefusal)?;
        Ok(Self {
            plan,
            play,
            fragment,
            plot_plan,
        })
    }
    pub fn plan(&self) -> &'a BodyPlan {
        self.plan
    }
    pub fn play(&self) -> &'a BodyPlayIdentity {
        self.play
    }
    pub fn plot_plan(&self) -> &'a conduit_core::Plan {
        self.plot_plan
    }
    pub fn fragment(&self) -> &'a PlanFragment {
        self.fragment
    }
}
