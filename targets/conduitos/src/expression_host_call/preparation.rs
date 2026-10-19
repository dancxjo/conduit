//! Immutable whole-fragment validation shared by exact expression owners.
use super::*;

/// A preparation-only proof over immutable Plan, routing and active-Play facts.
/// This context owns no scheduler state and grants no authority. Each owner
/// still checks its exact placement, program, selected offer and numeric bounds.
pub struct ExpressionPreparation<'a> {
    fragment: &'a PlanFragment,
    lowered: &'a LoweredPlanFragment,
    _active: &'a ActivePlayIdentity,
}
impl<'a> ExpressionPreparation<'a> {
    pub fn new(
        fragment: &'a PlanFragment,
        lowered: &'a LoweredPlanFragment,
        active: &'a ActivePlayIdentity,
    ) -> Result<Self, ExpressionCallRefusal> {
        use ExpressionCallRefusal as Refusal;
        if !verify_plan_fragment(fragment)
            || active.plan_id != fragment.plan_id
            || active.host_id != fragment.host_id
            || active.boot_id != fragment.boot_id
            || bind_active_play(
                &fragment.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                active.play_sequence,
            ) != *active
            || lower_plan_fragment(fragment).map_err(|_| Refusal::WrongBinding)? != *lowered
        {
            return Err(Refusal::WrongBinding);
        }
        Ok(Self {
            fragment,
            lowered,
            _active: active,
        })
    }
    pub fn prepare(
        &self,
        placement: &PlacementId,
    ) -> Result<ExpressionHostCall, ExpressionCallRefusal> {
        use ExpressionCallRefusal as Refusal;
        let fragment = self.fragment;
        let lowered = self.lowered;
        let mut gears = fragment
            .placements
            .iter()
            .filter(|gear| &gear.placement_id == placement);
        let gear = gears.next().ok_or(Refusal::WrongBinding)?;
        if gears.next().is_some() {
            return Err(Refusal::WrongBinding);
        }
        if gear.host_id != fragment.host_id || gear.boot_id != fragment.boot_id {
            return Err(Refusal::WrongBinding);
        }
        let program = prepared_program(gear)?;
        let mut nodes = lowered
            .identity
            .placements
            .iter()
            .filter(|(_, id)| id == placement);
        let node = nodes.next().ok_or(Refusal::WrongBinding)?.0;
        if nodes.next().is_some() {
            return Err(Refusal::WrongBinding);
        }
        Ok(ExpressionHostCall {
            evaluator: PreparedPortableExpressionEvaluator::new(&program)
                .map_err(Refusal::Evaluation)?,
            node,
            next_request: 0,
            maximum_input_bytes: program
                .maximum_prepared_input_bytes()
                .map_err(Refusal::Evaluation)?,
            cancelled: false,
        })
    }
}
