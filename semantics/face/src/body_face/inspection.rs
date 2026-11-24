//! Navigation to a resident inspection with one selected contribution.
use super::{validation::play_is_current, *};
use conduit_body::BodyPlan;

impl BodyFace {
    /// Offer inspection routes from the exact current Body Plan while
    /// projecting only the contribution selected for this Face.
    #[allow(clippy::too_many_arguments)]
    pub fn project_with_inspection_routes(
        body: &Body,
        wake: &Wake,
        plan: &BodyPlan,
        revision: u64,
        context: FaceContext,
        focus: FaceFocus,
        contributions: Vec<FaceContribution>,
        names: FaceNames<'_>,
        routes: &[&FaceContributionBasis],
    ) -> Result<Self, FaceRefusal> {
        validate_wake(body, Some(wake))?;
        if routes.len() > body.workset.plots().len() {
            return Err(FaceRefusal::TooManyContributions);
        }
        plan.validate_for_borrowed(wake)
            .map_err(|_| FaceRefusal::PlayNotCurrent)?;
        for route in routes {
            if !plan
                .plots
                .iter()
                .any(|plot| plot.plot.checked_plot_id == route.checked_plot_id)
            {
                return Err(FaceRefusal::PlotNotResident);
            }
            if route.plan_id != plan.plan_id
                || !play_is_current(Some(wake), &route.plan_id, &route.active_play_id)
            {
                return Err(FaceRefusal::PlayNotCurrent);
            }
        }
        Self::project_with_admission(
            body,
            Some(wake),
            revision,
            context,
            focus,
            contributions,
            names,
            None,
            routes,
        )
    }
}
