use super::MidiOutputSelection;
use std::fs::File;
use std::io::{self, Write};

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
    pub normalized_note_events: Vec<conduit_std_catalog::NormalizedNoteEvidence>,
    #[cfg(test)]
    pub encoded_messages: Vec<[u8; 3]>,
}

pub(crate) enum MidiOutputSession {
    Unsupported {
        lifecycle: MidiOutputLifecycle,
        normalized_note_events: Vec<conduit_std_catalog::NormalizedNoteEvidence>,
    },
    #[cfg(test)]
    Fake(super::output_fake::FakeMidiOutputSession),
    Raw {
        file: Option<File>,
        lifecycle: MidiOutputLifecycle,
        sent_messages: u16,
        all_notes_off_sent: bool,
        normalized_note_events: Vec<conduit_std_catalog::NormalizedNoteEvidence>,
    },
}

impl MidiOutputSession {
    pub(crate) fn prepare(selection: MidiOutputSelection) -> Result<Self, MidiOutputFailure> {
        #[cfg(test)]
        if let MidiOutputSelection::Sequencer(selection) = &selection {
            if let Some(behavior) = selection.fake_output {
                return Ok(Self::Fake(super::output_fake::FakeMidiOutputSession::new(
                    behavior,
                )));
            }
        }
        let MidiOutputSelection::Raw(selection) = selection else {
            return Ok(Self::Unsupported {
                lifecycle: MidiOutputLifecycle::Resolved,
                normalized_note_events: Vec::with_capacity(usize::from(
                    super::MAXIMUM_PENDING_MESSAGES,
                )),
            });
        };
        let path = selection
            .observation()
            .direct_device_path()
            .ok_or(MidiOutputFailure::BackendUnavailable)?;
        let file = open_nonblocking(&path).map_err(classify_open_error)?;
        Ok(Self::Raw {
            file: Some(file),
            lifecycle: MidiOutputLifecycle::Open,
            sent_messages: 0,
            all_notes_off_sent: false,
            normalized_note_events: Vec::with_capacity(usize::from(
                super::MAXIMUM_PENDING_MESSAGES,
            )),
        })
    }

    #[cfg(test)]
    pub(crate) fn prepare_test_raw(file: File) -> Self {
        Self::Raw {
            file: Some(file),
            lifecycle: MidiOutputLifecycle::Open,
            sent_messages: 0,
            all_notes_off_sent: false,
            normalized_note_events: Vec::with_capacity(usize::from(
                super::MAXIMUM_PENDING_MESSAGES,
            )),
        }
    }

    pub(crate) fn send_note(
        &mut self,
        event: conduit_core::MusicalNoteEvent,
        encoded: [u8; 3],
    ) -> Result<(), MidiOutputFailure> {
        self.send(encoded)?;
        let evidence = conduit_std_catalog::NormalizedNoteEvidence::exact(event);
        match self {
            Self::Unsupported {
                normalized_note_events,
                ..
            }
            | Self::Raw {
                normalized_note_events,
                ..
            } => normalized_note_events.push(evidence),
            #[cfg(test)]
            Self::Fake(session) => session.record_note(evidence)?,
        }
        Ok(())
    }

    pub(crate) fn send(&mut self, encoded: [u8; 3]) -> Result<(), MidiOutputFailure> {
        match self {
            Self::Unsupported { lifecycle, .. } => {
                let _ = encoded;
                *lifecycle = MidiOutputLifecycle::Failed;
                Err(MidiOutputFailure::BackendUnavailable)
            }
            #[cfg(test)]
            Self::Fake(session) => session.send(encoded),
            Self::Raw {
                file,
                lifecycle,
                sent_messages,
                ..
            } => {
                if *lifecycle != MidiOutputLifecycle::Open {
                    return Err(MidiOutputFailure::InvalidLifecycle);
                }
                let result = file
                    .as_mut()
                    .ok_or(MidiOutputFailure::InvalidLifecycle)?
                    .write(&encoded);
                match result {
                    Ok(3) => {
                        *sent_messages = sent_messages
                            .checked_add(1)
                            .ok_or(MidiOutputFailure::Pressure)?;
                        Ok(())
                    }
                    Ok(_) => {
                        *lifecycle = MidiOutputLifecycle::Failed;
                        Err(MidiOutputFailure::ProviderLost)
                    }
                    Err(error) => {
                        let failure = classify_write_error(&error);
                        if failure != MidiOutputFailure::Pressure {
                            *lifecycle = MidiOutputLifecycle::Failed;
                        }
                        Err(failure)
                    }
                }
            }
        }
    }

    pub(crate) fn stop(&mut self) -> Result<(), MidiOutputFailure> {
        match self {
            Self::Unsupported { lifecycle, .. } => {
                *lifecycle = MidiOutputLifecycle::StoppedClosed;
                Ok(())
            }
            #[cfg(test)]
            Self::Fake(session) => session.stop(),
            Self::Raw {
                file,
                lifecycle,
                sent_messages,
                all_notes_off_sent,
                ..
            } => {
                if *lifecycle == MidiOutputLifecycle::StoppedClosed {
                    return Ok(());
                }
                if *lifecycle == MidiOutputLifecycle::Failed {
                    file.take();
                    return Err(MidiOutputFailure::ProviderLost);
                }
                let all_notes_off = [0xb0 | super::OUTPUT_CHANNEL, 123, 0];
                let result = file
                    .as_mut()
                    .ok_or(MidiOutputFailure::InvalidLifecycle)?
                    .write(&all_notes_off);
                match result {
                    Ok(3) => {
                        *sent_messages = sent_messages
                            .checked_add(1)
                            .ok_or(MidiOutputFailure::Pressure)?;
                        *all_notes_off_sent = true;
                        file.take();
                        *lifecycle = MidiOutputLifecycle::StoppedClosed;
                        Ok(())
                    }
                    Ok(_) => {
                        file.take();
                        *lifecycle = MidiOutputLifecycle::Failed;
                        Err(MidiOutputFailure::ProviderLost)
                    }
                    Err(error) => {
                        let failure = classify_write_error(&error);
                        file.take();
                        *lifecycle = MidiOutputLifecycle::Failed;
                        Err(failure)
                    }
                }
            }
        }
    }

    pub(crate) fn report(&self) -> MidiOutputReport {
        match self {
            Self::Unsupported {
                lifecycle,
                normalized_note_events,
            } => MidiOutputReport {
                lifecycle: *lifecycle,
                sent_messages: 0,
                all_notes_off_sent: false,
                normalized_note_events: normalized_note_events.clone(),
                #[cfg(test)]
                encoded_messages: Vec::new(),
            },
            #[cfg(test)]
            Self::Fake(session) => session.report(),
            Self::Raw {
                lifecycle,
                sent_messages,
                all_notes_off_sent,
                normalized_note_events,
                ..
            } => MidiOutputReport {
                lifecycle: *lifecycle,
                sent_messages: *sent_messages,
                all_notes_off_sent: *all_notes_off_sent,
                normalized_note_events: normalized_note_events.clone(),
                #[cfg(test)]
                encoded_messages: Vec::new(),
            },
        }
    }
}

impl Drop for MidiOutputSession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(target_os = "linux")]
fn open_nonblocking(path: &str) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NONBLOCK: i32 = 0o4000;
    File::options()
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open(path)
}

#[cfg(not(target_os = "linux"))]
fn open_nonblocking(_path: &str) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "ALSA RawMIDI is available only on Linux",
    ))
}

fn classify_open_error(error: io::Error) -> MidiOutputFailure {
    match error.kind() {
        io::ErrorKind::WouldBlock => MidiOutputFailure::Pressure,
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
            MidiOutputFailure::BackendUnavailable
        }
        _ => MidiOutputFailure::ProviderLost,
    }
}

fn classify_write_error(error: &io::Error) -> MidiOutputFailure {
    if error.kind() == io::ErrorKind::WouldBlock {
        MidiOutputFailure::Pressure
    } else {
        MidiOutputFailure::ProviderLost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosted_midi::{
        HostedRawMidiSelection, MidiEndpointDirection, RawMidiEndpointObservation,
    };
    use conduit_core::{BootId, OfferGeneration};
    use std::io::{Read, Seek, SeekFrom};

    #[test]
    fn raw_session_writes_exact_messages_and_cleanup_before_close() {
        let path = std::env::temp_dir().join(format!(
            "conduit-raw-midi-output-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let mut reader = File::options()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let writer = reader.try_clone().unwrap();
        let mut session = MidiOutputSession::prepare_test_raw(writer);
        session.send([0x90, 60, 100]).unwrap();
        session.send([0x80, 60, 0]).unwrap();
        session.stop().unwrap();
        assert_eq!(
            session.report(),
            MidiOutputReport {
                lifecycle: MidiOutputLifecycle::StoppedClosed,
                sent_messages: 3,
                all_notes_off_sent: true,
                normalized_note_events: Vec::new(),
                encoded_messages: Vec::new(),
            }
        );
        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, [0x90, 60, 100, 0x80, 60, 0, 0xb0, 123, 0]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn dropping_an_open_session_attempts_all_notes_off_before_close() {
        let path = std::env::temp_dir().join(format!(
            "conduit-raw-midi-output-drop-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let mut reader = File::options()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        {
            let writer = reader.try_clone().unwrap();
            let mut session = MidiOutputSession::prepare_test_raw(writer);
            session.send([0x90, 60, 100]).unwrap();
        }
        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, [0x90, 60, 100, 0xb0, 123, 0]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pressure_and_provider_loss_remain_distinct() {
        assert_eq!(
            classify_write_error(&io::Error::from(io::ErrorKind::WouldBlock)),
            MidiOutputFailure::Pressure
        );
        assert_eq!(
            classify_write_error(&io::Error::from(io::ErrorKind::BrokenPipe)),
            MidiOutputFailure::ProviderLost
        );
        assert_eq!(
            classify_open_error(io::Error::from(io::ErrorKind::PermissionDenied)),
            MidiOutputFailure::BackendUnavailable
        );
    }

    #[test]
    fn exact_but_missing_raw_device_refuses_during_pre_play_preparation() {
        let observation = RawMidiEndpointObservation {
            card: u16::MAX,
            device: u16::MAX,
            subdevice: 0,
            name: "Absent proof endpoint".into(),
            direction: MidiEndpointDirection::WritableDestination,
        };
        let selection = HostedRawMidiSelection::select(
            &[observation],
            MidiEndpointDirection::WritableDestination,
            u16::MAX,
            u16::MAX,
            0,
            BootId::from("boot-proof"),
            OfferGeneration(9),
        )
        .unwrap();
        assert_eq!(
            MidiOutputSession::prepare(MidiOutputSelection::raw(selection)).err(),
            Some(MidiOutputFailure::BackendUnavailable)
        );
    }
}
