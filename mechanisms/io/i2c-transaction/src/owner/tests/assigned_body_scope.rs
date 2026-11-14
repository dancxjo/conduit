//! CPU-only ordinary Body context; none of these IDs grant physical I2C access.
use conduit_body::{
    Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, BodyWorkset, ResidentPlot, Wake,
};
use conduit_core::{ActivePlayId, Plan, PlanFragment, PlanId};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope, body_fragment_scope::PreparedBodyFragment,
};

pub(super) struct Context {
    wake: Wake,
    plan: BodyPlan,
    play: BodyPlayIdentity,
}
impl Context {
    pub(super) fn new(plot_plan: &Plan) -> Self {
        let plot = ResidentPlot::new(
            plot_plan.source_document_id.clone(),
            plot_plan.checked_plot_id.clone(),
        );
        let body = Body::born_with_plots(
            BodyWorkset::from_plots([plot.clone()]).unwrap(),
            1,
            "fixture/born".into(),
        )
        .unwrap();
        let wake = body.wake(1, "fixture/woke".into()).unwrap().1;
        let plan = BodyPlan::seal(
            &wake,
            alloc::vec![BodyPlotPlan {
                plot,
                plan: plot_plan.clone()
            }],
        )
        .unwrap();
        let play = BodyPlayIdentity::bind(&plan, 1);
        assert_ne!(plan.plan_id, plot_plan.plan_id);
        Self { wake, plan, play }
    }
    pub(super) fn scope<'a>(&'a self, fragment: &'a PlanFragment) -> AssignedPlayScope<'a> {
        AssignedPlayScope::Body(
            PreparedBodyFragment::prepare(&self.wake, &self.plan, &self.play, fragment).unwrap(),
        )
    }
    pub(super) fn plan_id(&self) -> &PlanId {
        &self.plan.plan_id
    }
    pub(super) fn play_id(&self) -> &ActivePlayId {
        &self.play.active_play_id
    }
}
