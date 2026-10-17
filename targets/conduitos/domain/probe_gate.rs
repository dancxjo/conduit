//! Architecture-neutral malformed effect requests for emulator evidence.
use crate::{frame::TextFrame, gate};

pub fn mutate_gate(frame: &mut TextFrame) {
    match frame.probe {
        8 => frame.capability = 0,
        9 => frame.capability = frame.target,
        10 => frame.operation += 1,
        11 => frame.input_length = 257,
        12 => frame.work_units = 2,
        13 => frame.capacity = 255,
        15 => frame.input[0] = 0xff,
        16 => gate::finish(0x10e), // A gate cannot forge a hardware fault origin.
        _ => {}
    }
}
