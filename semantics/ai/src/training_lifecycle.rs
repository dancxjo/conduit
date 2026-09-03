//! Finite lifecycle for one Host-owned training realization.

use super::TrainingRefusal;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TrainingLifecyclePhase {
    Unloaded,
    Loading,
    Ready,
    ActiveStep { step: u64 },
    Evaluating,
    Checkpointing,
    Cancelled,
    ProviderLost,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingLifecycle {
    pub session_identity: [u8; 32],
    pub phase: TrainingLifecyclePhase,
}

impl TrainingLifecycle {
    pub fn transition(&mut self, next: TrainingLifecyclePhase) -> Result<(), TrainingRefusal> {
        use TrainingLifecyclePhase::*;
        let permitted = matches!(
            (self.phase, next),
            (Unloaded, Loading)
                | (Loading, Ready | Failed | ProviderLost | Cancelled)
                | (
                    Ready,
                    ActiveStep { .. } | Evaluating | Checkpointing | Unloaded
                )
                | (ActiveStep { .. }, Ready | Failed | ProviderLost | Cancelled)
                | (Evaluating, Ready | Failed | ProviderLost | Cancelled)
                | (Checkpointing, Ready | Failed | ProviderLost | Cancelled)
                | (Cancelled | ProviderLost | Failed, Unloaded)
        );
        if self.session_identity == [0; 32] || matches!(next, ActiveStep { step: 0 }) || !permitted
        {
            return Err(TrainingRefusal::InvalidLifecycleTransition);
        }
        self.phase = next;
        Ok(())
    }
}
