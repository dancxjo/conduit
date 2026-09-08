//! Exact classification of scheduler drainage for an attached std Play.

use crate::RunControl;
use conduit_core::{PlanCompletionPolicy, PlanFragment};
use std::time::Duration;

pub(super) const fn drained_completes(fragment: &PlanFragment, attach_live: bool) -> bool {
    !attach_live
        || matches!(
            fragment.completion_policy,
            PlanCompletionPolicy::SemanticCompletion
        )
}

pub(super) fn await_live_control(control: &RunControl) {
    // The caller retains the scheduler, Plan, Play, stores, operations, and
    // resource envelope on its stack. This wait owns no execution machinery.
    control.mark_quiescent();
    std::thread::park_timeout(Duration::from_millis(1));
}
