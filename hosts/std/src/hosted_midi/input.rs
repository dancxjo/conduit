use super::{HostedRawMidiSelection, MidiEndpointDirection};
use conduit_midi::{MidiInputObservation, MidiParseError, MidiParser};
use std::fs::File;
use std::io::{self, Read};
use std::time::Duration;

pub const MAXIMUM_INPUT_BYTES_PER_POLL: usize = 32;
pub const MAXIMUM_INPUT_BYTES_PER_SESSION: u32 = 65_536;
pub const MAXIMUM_INPUT_PENDING_BYTES: u8 = (MAXIMUM_INPUT_BYTES_PER_POLL - 1) as u8;
pub const MAXIMUM_INPUT_PENDING_MESSAGES: u8 = (MAXIMUM_INPUT_BYTES_PER_POLL - 1) as u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiInputFailure {
    BackendUnavailable,
    ProviderLost,
    Malformed(MidiParseError),
    CapacityExceeded,
    ClockRegressed,
    InvalidLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiInputLifecycle {
    Open,
    CancelledClosed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiInputPoll {
    Pending,
    Observation(MidiInputObservation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiInputReport {
    pub lifecycle: MidiInputLifecycle,
    pub bytes_read: u32,
    pub observations: u16,
    pub pending_bytes: u8,
}

pub struct MidiInputSession {
    file: Option<File>,
    lifecycle: MidiInputLifecycle,
    parser: MidiParser,
    buffer: [u8; MAXIMUM_INPUT_BYTES_PER_POLL],
    buffer_cursor: u8,
    buffer_length: u8,
    buffer_time_micros: u64,
    last_now_micros: Option<u64>,
    bytes_read: u32,
    observations: u16,
    #[cfg(test)]
    fake_writer: Option<File>,
}

impl MidiInputSession {
    pub fn prepare(selection: &HostedRawMidiSelection) -> Result<Self, MidiInputFailure> {
        if selection.observation().direction != MidiEndpointDirection::ReadableSource {
            return Err(MidiInputFailure::BackendUnavailable);
        }
        #[cfg(test)]
        if let Some(bytes) = selection.fake_input() {
            return Self::prepare_test_pipe(bytes);
        }
        let path = selection
            .observation()
            .direct_device_path()
            .ok_or(MidiInputFailure::BackendUnavailable)?;
        let file = open_nonblocking(&path).map_err(classify_open_error)?;
        Ok(Self::from_file(file))
    }

    #[cfg(test)]
    fn prepare_test_raw(file: File) -> Self {
        Self::from_file(file)
    }

    pub fn poll(&mut self, now_micros: u64) -> Result<MidiInputPoll, MidiInputFailure> {
        if self.lifecycle != MidiInputLifecycle::Open {
            return Err(MidiInputFailure::InvalidLifecycle);
        }
        self.observe_now(now_micros)?;
        if self.buffer_cursor < self.buffer_length {
            return self.process_buffer();
        }
        if self.bytes_read == MAXIMUM_INPUT_BYTES_PER_SESSION {
            return self.fail(MidiInputFailure::CapacityExceeded);
        }
        let remaining = (MAXIMUM_INPUT_BYTES_PER_SESSION - self.bytes_read) as usize;
        let maximum = remaining.min(MAXIMUM_INPUT_BYTES_PER_POLL);
        let result = self
            .file
            .as_mut()
            .ok_or(MidiInputFailure::InvalidLifecycle)?
            .read(&mut self.buffer[..maximum]);
        match result {
            Ok(0) => match self.parser.finish() {
                Ok(()) => self.fail(MidiInputFailure::ProviderLost),
                Err(error) => self.fail(MidiInputFailure::Malformed(error)),
            },
            Ok(length) => {
                self.bytes_read = self
                    .bytes_read
                    .checked_add(length as u32)
                    .ok_or(MidiInputFailure::CapacityExceeded)?;
                self.buffer_cursor = 0;
                self.buffer_length = length as u8;
                self.buffer_time_micros = now_micros;
                self.process_buffer()
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(MidiInputPoll::Pending),
            Err(_) => self.fail(MidiInputFailure::ProviderLost),
        }
    }

    pub fn cancel(&mut self) {
        if self.lifecycle != MidiInputLifecycle::Open {
            return;
        }
        self.parser.cancel();
        self.buffer_cursor = 0;
        self.buffer_length = 0;
        self.file.take();
        #[cfg(test)]
        self.fake_writer.take();
        self.lifecycle = MidiInputLifecycle::CancelledClosed;
    }

    /// Waits for this exact descriptor to become readable for at most the
    /// advertised finite readiness interval. This is readiness only: parsing,
    /// timing, and kernel scheduling remain outside the platform adapter.
    #[cfg(unix)]
    pub fn wait_readable(&mut self, timeout: Duration) -> Result<bool, MidiInputFailure> {
        use std::os::fd::AsRawFd;

        if self.lifecycle != MidiInputLifecycle::Open {
            return Err(MidiInputFailure::InvalidLifecycle);
        }
        if self.buffer_cursor < self.buffer_length {
            return Ok(true);
        }
        let file = self
            .file
            .as_ref()
            .ok_or(MidiInputFailure::InvalidLifecycle)?;
        let mut descriptor = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
        let result = unsafe { libc::poll(&mut descriptor, 1, millis) };
        if result < 0 {
            return self.fail(MidiInputFailure::ProviderLost);
        }
        Ok(
            result > 0
                && (descriptor.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0,
        )
    }

    #[cfg(not(unix))]
    pub fn wait_readable(&mut self, _timeout: Duration) -> Result<bool, MidiInputFailure> {
        self.fail(MidiInputFailure::BackendUnavailable)
    }

    pub fn report(&self) -> MidiInputReport {
        MidiInputReport {
            lifecycle: self.lifecycle,
            bytes_read: self.bytes_read,
            observations: self.observations,
            pending_bytes: self
                .buffer_length
                .saturating_sub(self.buffer_cursor)
                .saturating_add(self.parser.pending_data_bytes()),
        }
    }

    fn from_file(file: File) -> Self {
        Self {
            file: Some(file),
            lifecycle: MidiInputLifecycle::Open,
            parser: MidiParser::new(),
            buffer: [0; MAXIMUM_INPUT_BYTES_PER_POLL],
            buffer_cursor: 0,
            buffer_length: 0,
            buffer_time_micros: 0,
            last_now_micros: None,
            bytes_read: 0,
            observations: 0,
            #[cfg(test)]
            fake_writer: None,
        }
    }

    fn process_buffer(&mut self) -> Result<MidiInputPoll, MidiInputFailure> {
        while self.buffer_cursor < self.buffer_length {
            let byte = self.buffer[usize::from(self.buffer_cursor)];
            self.buffer_cursor += 1;
            match self.parser.feed(byte) {
                Ok(Some(parsed)) => {
                    let Some(observations) = self.observations.checked_add(1) else {
                        return self.fail(MidiInputFailure::CapacityExceeded);
                    };
                    self.observations = observations;
                    return Ok(MidiInputPoll::Observation(MidiInputObservation {
                        event_time_micros: self.buffer_time_micros,
                        parsed,
                    }));
                }
                Ok(None) => {}
                Err(error) => return self.fail(MidiInputFailure::Malformed(error)),
            }
        }
        self.buffer_cursor = 0;
        self.buffer_length = 0;
        Ok(MidiInputPoll::Pending)
    }

    fn observe_now(&mut self, now_micros: u64) -> Result<(), MidiInputFailure> {
        if self
            .last_now_micros
            .is_some_and(|previous| now_micros < previous)
        {
            return self.fail(MidiInputFailure::ClockRegressed);
        }
        self.last_now_micros = Some(now_micros);
        Ok(())
    }

    fn fail<T>(&mut self, failure: MidiInputFailure) -> Result<T, MidiInputFailure> {
        self.parser.cancel();
        self.buffer_cursor = 0;
        self.buffer_length = 0;
        self.file.take();
        #[cfg(test)]
        self.fake_writer.take();
        self.lifecycle = MidiInputLifecycle::Failed;
        Err(failure)
    }

    #[cfg(all(test, unix))]
    fn prepare_test_pipe(bytes: &[u8]) -> Result<Self, MidiInputFailure> {
        use std::io::Write;
        use std::os::fd::FromRawFd;

        let mut descriptors = [0; 2];
        if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
            return Err(MidiInputFailure::BackendUnavailable);
        }
        let flags = unsafe { libc::fcntl(descriptors[0], libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(descriptors[0], libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0
        {
            unsafe {
                libc::close(descriptors[0]);
                libc::close(descriptors[1]);
            }
            return Err(MidiInputFailure::BackendUnavailable);
        }
        let reader = unsafe { File::from_raw_fd(descriptors[0]) };
        let mut writer = unsafe { File::from_raw_fd(descriptors[1]) };
        writer
            .write_all(bytes)
            .map_err(|_| MidiInputFailure::ProviderLost)?;
        let mut session = Self::from_file(reader);
        session.fake_writer = Some(writer);
        Ok(session)
    }

    #[cfg(all(test, not(unix)))]
    fn prepare_test_pipe(_bytes: &[u8]) -> Result<Self, MidiInputFailure> {
        Err(MidiInputFailure::BackendUnavailable)
    }
}

#[cfg(target_os = "linux")]
fn open_nonblocking(path: &str) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NONBLOCK: i32 = 0o4000;
    File::options()
        .read(true)
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

fn classify_open_error(error: io::Error) -> MidiInputFailure {
    match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
            MidiInputFailure::BackendUnavailable
        }
        _ => MidiInputFailure::ProviderLost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_midi::{MidiMessage, ParsedMidi};
    use std::io::{Seek, SeekFrom, Write};

    fn input_file(bytes: &[u8]) -> (File, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "conduit-raw-midi-input-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let mut file = File::options()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        (file, path)
    }

    #[test]
    fn one_bounded_read_retains_order_and_one_read_timestamp() {
        let (file, path) = input_file(&[0x90, 60, 100, 0x80, 60, 0, 0xb0, 64, 127]);
        let mut session = MidiInputSession::prepare_test_raw(file);
        let expected = [
            MidiMessage::NoteOn {
                channel: 0,
                key: 60,
                velocity: 100,
            },
            MidiMessage::NoteOff {
                channel: 0,
                key: 60,
                velocity: 0,
            },
            MidiMessage::ControlChange {
                channel: 0,
                controller: 64,
                value: 127,
            },
        ];
        for (index, message) in expected.into_iter().enumerate() {
            assert_eq!(
                session.poll(100 + index as u64),
                Ok(MidiInputPoll::Observation(MidiInputObservation {
                    event_time_micros: 100,
                    parsed: ParsedMidi::Message(message),
                }))
            );
        }
        assert_eq!(
            session.report(),
            MidiInputReport {
                lifecycle: MidiInputLifecycle::Open,
                bytes_read: 9,
                observations: 3,
                pending_bytes: 0,
            }
        );
        assert_eq!(session.poll(103), Err(MidiInputFailure::ProviderLost));
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn nonblocking_pressure_waits_without_consumption_then_observes_loss() {
        use std::os::fd::FromRawFd;

        let mut descriptors = [0; 2];
        // SAFETY: `pipe` initializes both descriptors on success. Each is
        // transferred exactly once into a `File` below.
        assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
        // SAFETY: the read descriptor is live and owned by this test.
        let flags = unsafe { libc::fcntl(descriptors[0], libc::F_GETFL) };
        assert!(flags >= 0);
        // SAFETY: the same live descriptor remains owned here; setting
        // O_NONBLOCK changes only its read disposition.
        assert_eq!(
            unsafe { libc::fcntl(descriptors[0], libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        // SAFETY: ownership of each fresh descriptor is transferred once.
        let reader = unsafe { File::from_raw_fd(descriptors[0]) };
        // SAFETY: ownership of the other fresh descriptor is transferred once.
        let mut writer = unsafe { File::from_raw_fd(descriptors[1]) };
        let mut session = MidiInputSession::prepare_test_raw(reader);

        assert_eq!(session.wait_readable(Duration::ZERO), Ok(false));
        assert_eq!(session.poll(10), Ok(MidiInputPoll::Pending));
        assert_eq!(session.report().bytes_read, 0);
        writer.write_all(&[0x90, 60, 100]).unwrap();
        assert_eq!(session.wait_readable(Duration::ZERO), Ok(true));
        assert!(matches!(
            session.poll(11),
            Ok(MidiInputPoll::Observation(MidiInputObservation {
                event_time_micros: 11,
                parsed: ParsedMidi::Message(MidiMessage::NoteOn { .. }),
            }))
        ));
        drop(writer);
        assert_eq!(session.wait_readable(Duration::ZERO), Ok(true));
        assert_eq!(session.poll(12), Err(MidiInputFailure::ProviderLost));
    }

    #[test]
    fn malformed_clock_capacity_and_cancel_are_distinct() {
        let (file, path) = input_file(&[0x90, 60]);
        let mut partial = MidiInputSession::prepare_test_raw(file);
        assert_eq!(partial.poll(4), Ok(MidiInputPoll::Pending));
        assert_eq!(partial.report().pending_bytes, 1);
        assert_eq!(
            partial.poll(5),
            Err(MidiInputFailure::Malformed(
                MidiParseError::DataByteExpected(1)
            ))
        );
        std::fs::remove_file(path).unwrap();

        let (file, path) = input_file(&[0x90, 60, 0x91]);
        let mut malformed = MidiInputSession::prepare_test_raw(file);
        assert_eq!(
            malformed.poll(5),
            Err(MidiInputFailure::Malformed(
                MidiParseError::DataByteExpected(1)
            ))
        );
        std::fs::remove_file(path).unwrap();

        let (file, path) = input_file(&[0x90, 60, 1]);
        let mut clock = MidiInputSession::prepare_test_raw(file);
        assert!(matches!(clock.poll(10), Ok(MidiInputPoll::Observation(_))));
        assert_eq!(clock.poll(9), Err(MidiInputFailure::ClockRegressed));
        std::fs::remove_file(path).unwrap();

        let (file, path) = input_file(&[0x90, 60, 1]);
        let mut capacity = MidiInputSession::prepare_test_raw(file);
        capacity.bytes_read = MAXIMUM_INPUT_BYTES_PER_SESSION - 1;
        assert_eq!(capacity.poll(1), Ok(MidiInputPoll::Pending));
        assert_eq!(capacity.poll(2), Err(MidiInputFailure::CapacityExceeded));
        std::fs::remove_file(path).unwrap();

        let (file, path) = input_file(&[0x90, 60, 1]);
        let mut cancelled = MidiInputSession::prepare_test_raw(file);
        cancelled.cancel();
        assert_eq!(
            cancelled.report().lifecycle,
            MidiInputLifecycle::CancelledClosed
        );
        assert_eq!(cancelled.poll(1), Err(MidiInputFailure::InvalidLifecycle));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn exact_missing_or_wrong_direction_selection_refuses_preparation() {
        use crate::hosted_midi::RawMidiEndpointObservation;
        use conduit_core::{BootId, OfferGeneration};

        for direction in [
            MidiEndpointDirection::ReadableSource,
            MidiEndpointDirection::WritableDestination,
        ] {
            let selection = HostedRawMidiSelection::select(
                &[RawMidiEndpointObservation {
                    card: u16::MAX,
                    device: u16::MAX,
                    subdevice: 0,
                    name: "Absent input".into(),
                    direction,
                }],
                direction,
                u16::MAX,
                u16::MAX,
                0,
                BootId::from("boot-input"),
                OfferGeneration(1),
            )
            .unwrap();
            assert_eq!(
                MidiInputSession::prepare(&selection).err(),
                Some(MidiInputFailure::BackendUnavailable)
            );
        }
    }
}
