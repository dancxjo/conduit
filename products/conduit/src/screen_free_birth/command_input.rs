//! A finite command queue lets a new user command interrupt an in-flight
//! spoken Play without reordering application actions.

use std::{
    io::{self, BufRead},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::spoken_face_mask::SpokenFaceSession;

use super::input::{parse_command, read_command_line};

pub(super) enum InputEvent {
    Line(String),
    Refused(&'static str),
    Eof,
    Io(String),
}

impl InputEvent {
    fn from_reader(input: &mut impl BufRead) -> Self {
        match read_command_line(input) {
            Ok(Ok(Some(line))) => Self::Line(line),
            Ok(Ok(None)) => Self::Eof,
            Ok(Err(reason)) => Self::Refused(reason),
            Err(error) => Self::Io(format!("terminal input: {error}")),
        }
    }
}

pub(super) trait CommandInput {
    fn next(&mut self) -> InputEvent;

    /// Only the selected spoken path consumes input during Play. The first
    /// An accepted queued command interrupts speech, then runs in exact order.
    /// Refused input stays queued until the current Play completes.
    fn interrupting_command(
        &mut self,
        _reader: &SpokenFaceSession,
        _face: &Presentation,
        _show: &MaskShow,
        _sequence: u64,
    ) -> bool {
        false
    }
}

pub(super) struct DirectInput<'a, R: BufRead>(pub(super) &'a mut R);

impl<R: BufRead> CommandInput for DirectInput<'_, R> {
    fn next(&mut self) -> InputEvent {
        InputEvent::from_reader(self.0)
    }
}

pub(super) struct SpokenInput {
    receiver: Receiver<InputEvent>,
    pending: Option<InputEvent>,
}

impl SpokenInput {
    #[cfg(test)]
    pub(super) fn from_receiver(receiver: Receiver<InputEvent>) -> Self {
        Self {
            receiver,
            pending: None,
        }
    }

    pub(super) fn from_stdin() -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("conduit-screen-free-input".into())
            .spawn(move || {
                let stdin = io::stdin();
                let mut input = stdin.lock();
                loop {
                    let event = InputEvent::from_reader(&mut input);
                    let terminal = matches!(event, InputEvent::Eof | InputEvent::Io(_));
                    if sender.send(event).is_err() || terminal {
                        break;
                    }
                }
            })
            .map_err(|error| format!("start bounded screen-free input: {error}"))?;
        Ok(Self {
            receiver,
            pending: None,
        })
    }
}

impl CommandInput for SpokenInput {
    fn next(&mut self) -> InputEvent {
        self.pending
            .take()
            .unwrap_or_else(|| self.receiver.recv().unwrap_or(InputEvent::Eof))
    }

    fn interrupting_command(
        &mut self,
        reader: &SpokenFaceSession,
        face: &Presentation,
        show: &MaskShow,
        sequence: u64,
    ) -> bool {
        if self.pending.is_some() {
            return false;
        }
        match self.receiver.try_recv() {
            Ok(InputEvent::Line(line)) if line == "stop" => true,
            Ok(InputEvent::Line(line)) => {
                let interrupts = line == "quit"
                    || parse_command(&line, reader, face).is_ok_and(|command| {
                        reader
                            .accepts_interruption(face, show, &command, sequence)
                            .is_ok()
                    });
                self.pending = Some(InputEvent::Line(line));
                interrupts
            }
            Ok(event @ InputEvent::Refused(_)) => {
                self.pending = Some(event);
                false
            }
            Ok(other) => {
                self.pending = Some(other);
                false
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.pending = Some(InputEvent::Eof);
                false
            }
        }
    }
}
