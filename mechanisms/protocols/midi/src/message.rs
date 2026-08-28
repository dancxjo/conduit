#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessage {
    NoteOff {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    NoteOn {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    PitchBend {
        channel: u8,
        value: u16,
    },
    UnsupportedChannel {
        status: u8,
        first: u8,
        second: Option<u8>,
    },
}

impl MidiMessage {
    pub(crate) fn decode(status: u8, data: [u8; 2], length: u8) -> Self {
        let channel = status & 0x0f;
        match status & 0xf0 {
            0x80 => Self::NoteOff {
                channel,
                key: data[0],
                velocity: data[1],
            },
            0x90 => Self::NoteOn {
                channel,
                key: data[0],
                velocity: data[1],
            },
            0xb0 => Self::ControlChange {
                channel,
                controller: data[0],
                value: data[1],
            },
            0xe0 => Self::PitchBend {
                channel,
                value: u16::from(data[0]) | (u16::from(data[1]) << 7),
            },
            _ => Self::UnsupportedChannel {
                status,
                first: data[0],
                second: (length == 2).then_some(data[1]),
            },
        }
    }
}
