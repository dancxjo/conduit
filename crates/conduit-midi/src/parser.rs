use crate::MidiMessage;

pub const MAXIMUM_SYSEX_BYTES: u16 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedMidi {
    Message(MidiMessage),
    UnsupportedSystem { status: u8 },
    UnsupportedRealtime { status: u8 },
    UnsupportedSysEx { bytes: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiParseError {
    UnexpectedData(u8),
    DataByteExpected(u8),
    SysExCapacityExceeded,
    SysExInterrupted(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MidiParser {
    running_status: Option<u8>,
    data: [u8; 2],
    data_length: u8,
    expected_length: u8,
    sysex_bytes: u16,
    in_sysex: bool,
}

impl MidiParser {
    pub const fn new() -> Self {
        Self {
            running_status: None,
            data: [0; 2],
            data_length: 0,
            expected_length: 0,
            sysex_bytes: 0,
            in_sysex: false,
        }
    }

    pub fn feed(&mut self, byte: u8) -> Result<Option<ParsedMidi>, MidiParseError> {
        if byte >= 0xf8 {
            return Ok(Some(ParsedMidi::UnsupportedRealtime { status: byte }));
        }
        if self.in_sysex {
            return self.feed_sysex(byte);
        }
        match byte {
            0xf0 => {
                self.running_status = None;
                self.data_length = 0;
                self.expected_length = 0;
                self.in_sysex = true;
                self.sysex_bytes = 1;
                Ok(None)
            }
            0xf1..=0xf7 => {
                self.running_status = None;
                self.data_length = 0;
                self.expected_length = 0;
                Ok(Some(ParsedMidi::UnsupportedSystem { status: byte }))
            }
            0x80..=0xef => {
                self.running_status = Some(byte);
                self.data_length = 0;
                self.expected_length = message_length(byte);
                Ok(None)
            }
            0x00..=0x7f => self.feed_data(byte),
            0xf8..=0xff => unreachable!("real-time status returned before the match"),
        }
    }

    pub fn finish(&mut self) -> Result<(), MidiParseError> {
        if self.in_sysex {
            self.reset();
            return Err(MidiParseError::SysExInterrupted(0));
        }
        if self.data_length != 0 {
            let missing = self.expected_length - self.data_length;
            self.reset();
            return Err(MidiParseError::DataByteExpected(missing));
        }
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.reset();
    }

    fn feed_data(&mut self, byte: u8) -> Result<Option<ParsedMidi>, MidiParseError> {
        let Some(status) = self.running_status else {
            return Err(MidiParseError::UnexpectedData(byte));
        };
        self.data[usize::from(self.data_length)] = byte;
        self.data_length += 1;
        if self.data_length != self.expected_length {
            return Ok(None);
        }
        let message = MidiMessage::decode(status, self.data, self.expected_length);
        self.data_length = 0;
        Ok(Some(ParsedMidi::Message(message)))
    }

    fn feed_sysex(&mut self, byte: u8) -> Result<Option<ParsedMidi>, MidiParseError> {
        if byte == 0xf7 {
            let bytes = self.sysex_bytes.saturating_add(1);
            self.reset();
            return Ok(Some(ParsedMidi::UnsupportedSysEx { bytes }));
        }
        if byte & 0x80 != 0 {
            self.reset();
            return Err(MidiParseError::SysExInterrupted(byte));
        }
        self.sysex_bytes = self.sysex_bytes.saturating_add(1);
        if self.sysex_bytes > MAXIMUM_SYSEX_BYTES {
            self.reset();
            return Err(MidiParseError::SysExCapacityExceeded);
        }
        Ok(None)
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}

const fn message_length(status: u8) -> u8 {
    match status & 0xf0 {
        0xc0 | 0xd0 => 1,
        _ => 2,
    }
}
