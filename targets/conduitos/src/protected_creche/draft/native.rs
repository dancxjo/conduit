//! Guest-owned native keyboard/controller projection.
use super::*;
use conduit_human::KeyEvent;
use conduitos_creche_core::native::NativeOutcome;
impl CrecheDraft {
    pub(crate) fn native_focus(&self) -> usize {
        self.snapshot.native_focus as usize
    }
    pub(crate) fn native_refusal(&self) -> Option<&str> {
        self.snapshot.native_refusal.as_deref()
    }
    pub(crate) fn accept_native(
        &mut self,
        event: KeyEvent,
    ) -> Result<NativeOutcome, TransportError> {
        let result = self.native_exchange(event);
        if result.is_err() {
            self.service.get_mut().quarantine();
        }
        result
    }
    fn native_exchange(&mut self, event: KeyEvent) -> Result<NativeOutcome, TransportError> {
        let previous = self.snapshot.revision;
        let response = self.service.get_mut().request(Request::NativeKey {
            revision: self.snapshot.revision,
            event: event.encode(),
        })?;
        let Response::Blob {
            kind: OutputKind::Native,
            revision,
            ..
        } = response
        else {
            return self.protocol();
        };
        let bytes = self
            .service
            .get_mut()
            .blob(response, OutputKind::Native, revision)?;
        let outcome: NativeOutcome = decode(self.service.get_mut(), &bytes)?;
        if revision != previous && previous.checked_add(1) != Some(revision) {
            return self.protocol();
        }
        if !matches!(outcome, NativeOutcome::Changed) && revision != previous {
            return self.protocol();
        }
        self.snapshot = fetch_snapshot(self.service.get_mut(), revision, &self.reviewed)?;
        match outcome {
            NativeOutcome::Birth(selection) => {
                self.bind_selection(selection).map(NativeOutcome::Birth)
            }
            other => Ok(other),
        }
    }
}
