//! Physical input keeps the original prepared SDL claim and current owner route.
use super::*;
use conduit_linux_display::{PreparedNativeInput, Scanout};
use conduit_semantic_catalog::NormalizedPointerSample;

#[derive(Default)]
pub(super) struct NativePointerState {
    sequence: Option<u64>,
    pressed: bool,
    sample_sequence: Option<u64>,
}
impl NativePointerState {
    fn sequence(&mut self, sequence: u64) -> Result<(), String> {
        if self.sequence.is_some_and(|previous| sequence <= previous) {
            return Err("native input sequence is stale".into());
        }
        self.sequence = Some(sequence);
        Ok(())
    }
    fn pointer(&mut self, sample: &NormalizedPointerSample) -> Result<bool, String> {
        if !(0..=1_000_000).contains(&sample.position_x)
            || !(0..=1_000_000).contains(&sample.position_y)
            || sample.dropped != 0
            || sample.coalesced != 0
            || sample.queue_capacity != 1
            || self
                .sample_sequence
                .is_some_and(|previous| sample.sequence <= previous)
        {
            return Err("native pointer bounds or transition custody differs".into());
        }
        let activation = sample.primary_pressed && !self.pressed;
        self.pressed = sample.primary_pressed;
        self.sample_sequence = Some(sample.sequence);
        Ok(activation)
    }
}
impl StdHost {
    /// Genuine initialized provider capability; a current Show/owner route is
    /// still required separately for each input occurrence.
    pub fn selected_native_mask_supports_input(&self) -> bool {
        self.selected_scanout
            .as_ref()
            .and_then(|s| s.native_mask.as_ref())
            .is_some_and(|mask| mask.input.as_ref().is_some_and(|claim| claim.is_live()))
    }
    /// Shared input-readiness metadata only, already included in the fixed
    /// receipt. SDL queue/driver preparation and mutable Scene peak are separate.
    pub fn selected_native_mask_input_owner_bytes(&self) -> Option<usize> {
        self.selected_native_mask_supports_input()
            .then_some(PreparedNativeInput::owned_backing_bytes())
    }
    // Keep physical input, route and occurrence witnesses distinct.
    #[allow(clippy::too_many_arguments)]
    pub fn selected_native_mask_pointer(
        &mut self,
        encounter: &impl NativeMaskEncounter,
        route: &RemoteOwnerMaskRouteSeal,
        input: &PreparedNativeInput,
        sample: NormalizedPointerSample,
        activate: bool,
        sequence: u64,
        output: &mut Scanout<'_>,
    ) -> Result<SelectedNativeMaskInput, String> {
        self.selected_native_input(encounter, route, input, output, |mask, output| {
            mask.pointer.sequence(sequence)?;
            let rising = mask.pointer.pointer(&sample)?;
            let activation = activate && rising;
            match &mut mask.stores {
                Stores::Bound { renderer, .. } => {
                    let (width, height) = renderer.input_geometry();
                    let x = (sample.position_x as u64 * u64::from(width.saturating_sub(1))
                        / 1_000_000) as u32;
                    let y = (sample.position_y as u64 * u64::from(height.saturating_sub(1))
                        / 1_000_000) as u32;
                    renderer.pointer(x, y, sequence, activation, output)
                }
                _ => Err("native input binding absent".into()),
            }
        })
    }
    pub fn selected_native_mask_key(
        &mut self,
        encounter: &impl NativeMaskEncounter,
        route: &RemoteOwnerMaskRouteSeal,
        input: &PreparedNativeInput,
        event: conduit_human::KeyEvent,
        sequence: u64,
        output: &mut Scanout<'_>,
    ) -> Result<SelectedNativeMaskInput, String> {
        self.selected_native_input(encounter, route, input, output, |mask, output| {
            mask.pointer.sequence(sequence)?;
            match &mut mask.stores {
                Stores::Bound { renderer, .. } => renderer.key(event, sequence, output),
                _ => Err("native input binding absent".into()),
            }
        })
    }
    fn selected_native_input(
        &mut self,
        encounter: &impl NativeMaskEncounter,
        route: &RemoteOwnerMaskRouteSeal,
        input: &PreparedNativeInput,
        output: &mut Scanout<'_>,
        operation: impl FnOnce(
            &mut SelectedNativeMaskBase,
            &mut Scanout<'_>,
        ) -> Result<SelectedNativeMaskInput, String>,
    ) -> Result<SelectedNativeMaskInput, String> {
        let scanout = self
            .ready_selected_scanout()
            .ok_or("selected output lost")?;
        if !output.matches_initialized(scanout.initialized())
            || !input.matches_display(scanout.initialized())
        {
            return Err("native input/output differs from original physical claim".into());
        }
        let mask = self
            .selected_scanout
            .as_mut()
            .and_then(|s| s.native_mask.as_mut())
            .ok_or("selected Mask absent")?;
        let claim = mask.input.as_ref().ok_or("genuine prepared input absent")?;
        if !claim.is_live() || !claim.prepared().same_instance(input) {
            return Err("native input differs from original prepared owner".into());
        }
        let Stores::Bound { renderer, .. } = &mask.stores else {
            return Err("native input binding absent".into());
        };
        let face = renderer
            .input_face()
            .ok_or("native input has no acknowledged Show")?;
        validate_encounter(encounter, route, &self.advertisement, face)?;
        operation(mask, output)
    }
    /// Dispose the exact completed local input custody after the semantic owner
    /// response. An owner refusal does not replay the consumed occurrence.
    pub fn retire_selected_native_interaction(
        &mut self,
        interaction: &mut Option<SelectedNativeInteraction>,
    ) -> Result<(), String> {
        let current = interaction.as_ref().ok_or("native interaction absent")?;
        if !current
            .evidence
            .as_ref()
            .is_some_and(|e| e.matches_host(self))
        {
            return Err(
                "native interaction belongs to another original Host/Boot/ledger; retained".into(),
            );
        }
        let SelectedNativeInteraction {
            correlation,
            show,
            mut evidence,
        } = interaction.take().ok_or("native interaction absent")?;
        drop(correlation);
        drop(show);
        self.retire_selected_native_mask_evidence(&mut evidence)
    }
}
