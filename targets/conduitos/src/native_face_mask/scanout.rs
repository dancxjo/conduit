//! Pixel writes and buffered display submission form one acknowledgement boundary.
use super::*;
impl NativeFaceMask {
    pub(super) fn compose_and_commit(
        &mut self,
        display: &mut impl PixelTarget,
    ) -> Result<(), NativeFaceMaskError> {
        let result = self
            .compositor
            .compose_frame(display)
            .map(|_| ())
            .and_then(|()| {
                display
                    .commit_frame()
                    .map_err(crate::native_compositor::NativeCompositorError::Display)
            });
        if let Err(error) = result {
            // Neither CPU coverage nor a preceding Show survives failed output.
            // Dropping the pending interaction session cancels its Mask Play.
            self.session.take();
            // Keep the forwarded semantic Face/source receipt for inspection
            // and recovery. It is not an available renderer acknowledgement;
            // all input routes require the session revoked above.
            self.interaction_admitted = false;
            if self.surface_admitted {
                self.compositor
                    .remove_surface(&self.surface_id)
                    .map_err(NativeFaceMaskError::Compositor)?;
                self.surface_admitted = false;
            }
            return Err(NativeFaceMaskError::Compositor(error));
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "scanout_tests.rs"]
mod tests;
