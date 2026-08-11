use super::HostedMidiSelection;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MidiOutputFailure {
    BackendUnavailable,
    Pressure,
    ProviderLost,
    InvalidLifecycle,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MidiOutputLifecycle {
    Resolved,
    Open,
    StoppedClosed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiOutputReport {
    pub lifecycle: MidiOutputLifecycle,
    pub sent_messages: u16,
    pub all_notes_off_sent: bool,
    #[cfg(test)]
    pub encoded_messages: Vec<[u8; 3]>,
}

pub(crate) enum MidiOutputSession {
    Unsupported {
        lifecycle: MidiOutputLifecycle,
    },
    #[cfg(test)]
    Fake(super::output_fake::FakeMidiOutputSession),
}

impl MidiOutputSession {
    pub(crate) fn resolved(selection: HostedMidiSelection) -> Self {
        #[cfg(test)]
        if let Some(behavior) = selection.fake_output {
            return Self::Fake(super::output_fake::FakeMidiOutputSession::new(behavior));
        }
        let _ = selection;
        Self::Unsupported {
            lifecycle: MidiOutputLifecycle::Resolved,
        }
    }

    pub(crate) fn send(&mut self, encoded: [u8; 3]) -> Result<(), MidiOutputFailure> {
        match self {
            Self::Unsupported { lifecycle } => {
                let _ = encoded;
                *lifecycle = MidiOutputLifecycle::Failed;
                Err(MidiOutputFailure::BackendUnavailable)
            }
            #[cfg(test)]
            Self::Fake(session) => session.send(encoded),
        }
    }

    pub(crate) fn stop(&mut self) -> Result<(), MidiOutputFailure> {
        match self {
            Self::Unsupported { lifecycle } => {
                *lifecycle = MidiOutputLifecycle::StoppedClosed;
                Ok(())
            }
            #[cfg(test)]
            Self::Fake(session) => session.stop(),
        }
    }

    pub(crate) fn report(&self) -> MidiOutputReport {
        match self {
            Self::Unsupported { lifecycle } => MidiOutputReport {
                lifecycle: *lifecycle,
                sent_messages: 0,
                all_notes_off_sent: false,
                #[cfg(test)]
                encoded_messages: Vec::new(),
            },
            #[cfg(test)]
            Self::Fake(session) => session.report(),
        }
    }
}
