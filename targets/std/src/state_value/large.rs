//! Exact preparation binding for the separately selected large Play-lived State.
use super::LargeTypedStateBack;
use conduit_core::{ActivePlayIdentity, PlanFragment};
use conduit_plan_lowering::lowering::LoweredState;

impl LargeTypedStateBack {
    pub fn prepare_for_play(
        fragment: &PlanFragment,
        state: &LoweredState,
        play: &ActivePlayIdentity,
    ) -> Result<Self, String> {
        let (placement, binding) = super::continuity::bind(fragment, state, play)?;
        let mut back = Self::prepare(
            placement,
            &state.contract,
            state.slot,
            state.next,
            state.current,
        )?;
        back.binding = Some(binding);
        Ok(back)
    }
}
