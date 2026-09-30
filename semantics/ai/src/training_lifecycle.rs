//! Finite lifecycle for one host-owned training realization.

use super::TrainingRefusal;
use crate::TrainingLifecyclePhase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingLifecycle {
    pub session_identity: [u8; 32],
    pub phase: TrainingLifecyclePhase,
}

impl TrainingLifecycle {
    pub fn transition(&mut self, next: TrainingLifecyclePhase) -> Result<(), TrainingRefusal> {
        use TrainingLifecyclePhase::*;
        let permitted = matches!(
            (&self.phase, &next),
            (Unloaded, Loading)
                | (Loading, Ready | Failed | ProviderLost | Cancelled)
                | (Ready, ActiveStep(_) | Evaluating | Checkpointing | Unloaded)
                | (ActiveStep(_), Ready | Failed | ProviderLost | Cancelled)
                | (Evaluating, Ready | Failed | ProviderLost | Cancelled)
                | (Checkpointing, Ready | Failed | ProviderLost | Cancelled)
                | (Cancelled | ProviderLost | Failed, Unloaded)
        );
        if self.session_identity == [0; 32]
            || matches!(&next, ActiveStep(active_step) if *active_step.step() == 0)
            || !permitted
        {
            return Err(TrainingRefusal::InvalidLifecycleTransition);
        }
        self.phase = next;
        Ok(())
    }
}
