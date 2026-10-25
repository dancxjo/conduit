//! Private receipts created only by the owned renderer, never caller ACK bytes.
use super::*;
use std::{
    sync::{Condvar, Mutex, MutexGuard},
    time::Instant,
};
struct Presented {
    face: Arc<Face>,
    show: MaskShow,
    render: NativeMaskPlayReceipt,
    composition: CompositionReceipt,
}
#[derive(Default)]
struct State {
    presented: Option<Presented>,
    returned: Option<MaskInteractionCorrelation>,
    revoked: bool,
    requested: bool,
    drained: bool,
    failure: Option<String>,
}
pub(super) struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    timeout: Duration,
}
impl Shared {
    pub(super) fn new(timeout: Duration) -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            timeout,
        }
    }
    fn lock(&self) -> Result<MutexGuard<'_, State>, String> {
        self.state
            .lock()
            .map_err(|_| "native Creche renderer custody poisoned".into())
    }
    pub(super) fn require_live(&self) -> Result<(), String> {
        if self.lock()?.revoked {
            Err("native Creche renderer custody revoked".into())
        } else {
            Ok(())
        }
    }
    pub(super) fn same_face(&self, face: &Face) -> Result<bool, String> {
        Ok(self
            .lock()?
            .presented
            .as_ref()
            .is_some_and(|p| p.face.as_ref() == face))
    }
    pub(super) fn has_returned(&self) -> Result<bool, String> {
        Ok(self.lock()?.returned.is_some())
    }
    pub(super) fn face(&self) -> Result<Arc<Face>, String> {
        self.lock()?
            .presented
            .as_ref()
            .map(|p| p.face.clone())
            .ok_or("native Creche actual presented Face absent".into())
    }
    pub(super) fn withdraw(&self) -> Result<(), String> {
        let mut state = self.lock()?;
        state.presented = None;
        state.returned = None;
        Ok(())
    }
    pub(super) fn record_presented(
        &self,
        face: Arc<Face>,
        show: MaskShow,
        render: NativeMaskPlayReceipt,
        composition: CompositionReceipt,
    ) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.revoked {
            return Err("native Creche renderer retired during presentation".into());
        }
        show.validate(&face)
            .map_err(|e| format!("native Creche renderer Show: {e:?}"))?;
        if show.show.lifecycle != conduit_face::ManifestationLifecycle::Available
            || render.face_id != face.identity.as_str()
            || render.face_revision != face.revision
            || render.mask_plan_id != show.show.plan_id
            || render.active_play_id != show.show.active_play_id
            || render.scanout_pixels_written == 0
            || render.display_base_id != composition.display_base_id
            || composition.face_id != face.identity
            || composition.plan_id != show.show.plan_id
            || composition.active_play_id != show.show.active_play_id
            || composition.manifestation_id != show.show.manifestation_id
        {
            return Err("native Creche renderer receipts disagree with completed Show".into());
        }
        state.presented = Some(Presented {
            face,
            show,
            render,
            composition,
        });
        state.returned = None;
        Ok(())
    }
    pub(super) fn record_returned(
        &self,
        face: &Face,
        show: &MaskShow,
        render: &NativeMaskPlayReceipt,
        returned: MaskInteractionCorrelation,
    ) -> Result<(), String> {
        let mut state = self.lock()?;
        validate(&state, face, show)?;
        if state.returned.is_some()
            || state.presented.as_ref().is_none_or(|p| &p.render != render)
            || returned.show_id != show.show_id
            || returned.mask_plot != show.mask_plot
            || returned.face_id != face.identity
            || returned.face_revision != face.revision
        {
            return Err(
                "native Creche typed return differs from its actual renderer session".into(),
            );
        }
        returned
            .interaction
            .validate_against(face, show)
            .map_err(|e| format!("native Creche typed return: {e:?}"))?;
        state.returned = Some(returned);
        Ok(())
    }
    pub(super) fn retirement_requested(&self) -> Result<bool, String> {
        Ok(self.lock()?.requested)
    }
    pub(super) fn is_drained(&self) -> Result<bool, String> {
        Ok(self.lock()?.drained)
    }
    pub(super) fn revoke(&self) -> Result<(), String> {
        let mut state = self.lock()?;
        state.revoked = true;
        state.presented = None;
        state.returned = None;
        Ok(())
    }
    pub(super) fn complete_retirement(&self, failure: Option<String>) -> Result<(), String> {
        let mut state = self.lock()?;
        state.revoked = true;
        state.presented = None;
        state.returned = None;
        state.drained = failure.is_none();
        state.requested = false;
        state.failure = failure.map(|message| message.chars().take(512).collect());
        self.changed.notify_all();
        Ok(())
    }
}
fn validate(state: &State, face: &Face, show: &MaskShow) -> Result<(), String> {
    if state.revoked {
        return Err("native Creche selected renderer has been revoked".into());
    }
    let p = state
        .presented
        .as_ref()
        .ok_or("native Creche has no actual selected presentation")?;
    if p.face.as_ref() != face || &p.show != show || p.composition.face_id != face.identity {
        return Err("native Creche Face/Show differs from actual selected renderer".into());
    }
    Ok(())
}
pub(super) struct NativeCustody(pub(super) Arc<Shared>);
impl CrecheMaskShowCustody for NativeCustody {
    fn validate_acknowledged(&mut self, face: &Face, show: &MaskShow) -> Result<(), String> {
        let state = self.0.lock()?;
        validate(&state, face, show)
    }
    fn validate_returned_interaction(
        &mut self,
        face: &Face,
        show: &MaskShow,
        interaction: &conduit_face::FaceInteraction,
    ) -> Result<(), String> {
        let state = self.0.lock()?;
        validate(&state, face, show)?;
        if !state
            .returned
            .as_ref()
            .is_some_and(|r| &r.interaction == interaction && r.show_id == show.show_id)
        {
            return Err("native Creche occurrence was not returned by selected Mask Fore".into());
        }
        Ok(())
    }
    fn retire_and_drain(&mut self) -> Result<(), String> {
        let mut state = self.0.lock()?;
        if state.drained {
            return Ok(());
        }
        state.revoked = true;
        state.presented = None;
        state.returned = None;
        state.failure = None;
        state.requested = true;
        self.0.changed.notify_all();
        let deadline = Instant::now() + self.0.timeout;
        loop {
            if state.drained {
                return Ok(());
            }
            if let Some(failure) = &state.failure {
                return Err(failure.clone());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("native Creche UI drain acknowledgement timed out".into());
            }
            state = self
                .0
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| "native Creche drain custody poisoned")?
                .0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retirement_waits_for_explicit_ui_drain_and_revokes_before_ack() {
        let shared = Arc::new(Shared::new(Duration::from_secs(1)));
        let worker_shared = shared.clone();
        let worker = std::thread::spawn(move || NativeCustody(worker_shared).retire_and_drain());
        let deadline = Instant::now() + Duration::from_secs(1);
        while !shared.retirement_requested().unwrap() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(shared.require_live().is_err());
        assert!(!shared.is_drained().unwrap());
        // This is a handshake test, not an actual renderer/drain proof.
        shared.complete_retirement(None).unwrap();
        worker.join().unwrap().unwrap();
        assert!(shared.is_drained().unwrap());
        NativeCustody(shared).retire_and_drain().unwrap();
    }
    #[test]
    fn missing_ui_drain_is_a_failure_and_cannot_restore_authority() {
        let shared = Arc::new(Shared::new(Duration::from_millis(1)));
        assert!(NativeCustody(shared.clone()).retire_and_drain().is_err());
        assert!(shared.require_live().is_err());
        assert!(!shared.is_drained().unwrap());
        assert!(!shared.has_returned().unwrap());
    }
}
