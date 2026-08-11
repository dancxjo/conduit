use super::{MidiOutputFailure, MidiOutputLifecycle, MidiOutputReport};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum FakeMidiOutputBehavior {
    Healthy,
    FailAfter(u16),
}

pub(crate) struct FakeMidiOutputSession {
    behavior: FakeMidiOutputBehavior,
    lifecycle: MidiOutputLifecycle,
    messages: Vec<[u8; 3]>,
    all_notes_off_sent: bool,
}

impl FakeMidiOutputSession {
    pub(crate) fn new(behavior: FakeMidiOutputBehavior) -> Self {
        Self {
            behavior,
            lifecycle: MidiOutputLifecycle::Resolved,
            messages: Vec::with_capacity(usize::from(super::MAXIMUM_PENDING_MESSAGES) + 1),
            all_notes_off_sent: false,
        }
    }

    pub(crate) fn send(&mut self, encoded: [u8; 3]) -> Result<(), MidiOutputFailure> {
        if matches!(
            self.lifecycle,
            MidiOutputLifecycle::StoppedClosed | MidiOutputLifecycle::Failed
        ) {
            return Err(MidiOutputFailure::InvalidLifecycle);
        }
        if let FakeMidiOutputBehavior::FailAfter(limit) = self.behavior {
            if self.messages.len() >= usize::from(limit) {
                self.lifecycle = MidiOutputLifecycle::Failed;
                return Err(MidiOutputFailure::ProviderLost);
            }
        }
        if self.messages.len() >= usize::from(super::MAXIMUM_PENDING_MESSAGES) {
            return Err(MidiOutputFailure::Pressure);
        }
        self.lifecycle = MidiOutputLifecycle::Open;
        self.messages.push(encoded);
        Ok(())
    }

    pub(crate) fn stop(&mut self) -> Result<(), MidiOutputFailure> {
        if self.lifecycle == MidiOutputLifecycle::StoppedClosed {
            return Ok(());
        }
        let all_notes_off = [0xb0 | super::OUTPUT_CHANNEL, 123, 0];
        if self.messages.len() >= self.messages.capacity() {
            self.lifecycle = MidiOutputLifecycle::Failed;
            return Err(MidiOutputFailure::Pressure);
        }
        self.messages.push(all_notes_off);
        self.all_notes_off_sent = true;
        self.lifecycle = MidiOutputLifecycle::StoppedClosed;
        Ok(())
    }

    pub(crate) fn report(&self) -> MidiOutputReport {
        MidiOutputReport {
            lifecycle: self.lifecycle,
            sent_messages: u16::try_from(self.messages.len()).unwrap_or(u16::MAX),
            all_notes_off_sent: self.all_notes_off_sent,
            encoded_messages: self.messages.clone(),
        }
    }
}
