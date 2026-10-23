//! Select an actual route's existing present call, without changing rendering.
use super::*;
use crate::protected_compositor::lifecycle::sign::Checkpoint;
impl FrontDoorPresenter {
    pub(crate) fn present_lifecycle_checkpoint(
        &mut self,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
        checkpoint: Checkpoint,
    ) -> Result<CompositionReceipt, PresenterError> {
        let selected = self.compositor.begin_lifecycle_checkpoint(checkpoint);
        let result = self.present(door, display);
        self.compositor
            .finish_lifecycle_checkpoint(checkpoint, selected, result.is_ok());
        result
    }
}
