//! Explicit replacement execution; source ownership remains visible on refusal.
use super::{HostRunInputs, RunControl, StdHost, TimerAdapter, Write};
use crate::state_value::{RetainedStdRun, RetainedTypedState};
use conduit_core::{PlanFragment, StateId};

/// A refused admission leaves all cells with the caller. If execution/preparation
/// failed after transfer, these exact cells are unavailable; no reset is implied.
#[derive(Debug)]
pub struct StateContinuationRunFailure {
    pub reason: String,
    pub unavailable_states: Vec<StateId>,
}

impl StdHost {
    pub fn run_fragment_continuing_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        sources: &mut Vec<RetainedTypedState>,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<RetainedStdRun, StateContinuationRunFailure> {
        if sources.is_empty() || sources.len() > 16 {
            return Err(StateContinuationRunFailure {
                reason: "continuation requires 1..=16 owned State cells".into(),
                unavailable_states: Vec::new(),
            });
        }
        let original = sources
            .iter()
            .map(|source| source.provenance().source_state.clone())
            .collect::<Vec<_>>();
        self.run_fragment_owned_with_keyboard_to(
            fragment,
            output,
            timer,
            control,
            HostRunInputs {
                keyboard: None,
                retained: Some(sources),
            },
        )
        .map_err(|reason| StateContinuationRunFailure {
            reason,
            unavailable_states: original
                .into_iter()
                .filter(|state| {
                    !sources
                        .iter()
                        .any(|source| &source.provenance().source_state == state)
                })
                .collect(),
        })
    }
}
