//! Route-local evidence selection; never a Show/input or renderer permission.
use super::*;
use crate::protected_compositor::lifecycle::sign::Checkpoint;
impl NativeFaceMask {
    pub(crate) fn begin_lifecycle_checkpoint(&self, checkpoint: Checkpoint) -> bool {
        self.compositor.begin_lifecycle_checkpoint(checkpoint)
    }
    pub(crate) fn finish_lifecycle_checkpoint(
        &self,
        checkpoint: Checkpoint,
        selected: bool,
        succeeded: bool,
    ) {
        self.compositor
            .finish_lifecycle_checkpoint(checkpoint, selected, succeeded);
    }
}
