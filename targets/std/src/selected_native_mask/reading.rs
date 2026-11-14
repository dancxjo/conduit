//! Fixed local reading cursor custody, independent of any finite Show or Plan.
use super::*;
use conduitos::native_face_scene::NativeReadingPosition;

/// Opaque local document position; contains no Show, action or executable route.
/// Its fixed inline owner and lease backing remain on the original IMAGE counter.
pub struct SelectedNativeReadingState {
    pub(super) position: Option<NativeReadingPosition>,
    heap: ImageMemoryLease,
}
impl SelectedNativeReadingState {
    pub fn owned_backing_bytes() -> usize {
        core::mem::size_of::<Self>() + ImageMemoryLease::owned_backing_bytes()
    }
}
impl StdHost {
    pub fn prepare_selected_native_reading_state(
        &mut self,
    ) -> Result<SelectedNativeReadingState, String> {
        if !self.selected_native_mask_supports_input() {
            return Err("reading cursor requires original selected native input".into());
        }
        let heap = self
            .kernel_resources
            .reserve_managed_base_memory(SelectedNativeReadingState::owned_backing_bytes())?;
        Ok(SelectedNativeReadingState {
            position: None,
            heap,
        })
    }
    /// Capture before retiring frame/Plan custody. No active Scene leaves the
    /// previous cookie intact, permitting finite carrier renewal without replay.
    pub fn capture_selected_native_reading_state(
        &self,
        state: &mut SelectedNativeReadingState,
    ) -> Result<(), String> {
        self.kernel_resources
            .require_input_memory_owner(&state.heap)?;
        if let Some(Stores::Bound { renderer, .. }) =
            self.ready_selected_native_mask().map(|m| &m.stores)
        {
            if let Some(scene) = &renderer.scene {
                state.position = scene.reading_position();
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn render_selected_interactive_native_mask_with_reading_state(
        &mut self,
        encounter: &impl NativeMaskEncounter,
        route: &RemoteOwnerMaskRouteSeal,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        output: &mut conduit_linux_display::Scanout<'_>,
        state: &mut SelectedNativeReadingState,
    ) -> Result<SelectedNativeMaskFrame, String> {
        self.kernel_resources
            .require_input_memory_owner(&state.heap)?;
        self.render_selected_native_mask_profile(
            encounter,
            route,
            face,
            play_sequence,
            front_subject,
            output,
            Some(()),
            state.position,
        )
    }
}
