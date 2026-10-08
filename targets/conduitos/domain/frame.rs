//! One bounded copy window shared by Root and the separately linked image.

pub const TEXT_CAPACITY: usize = 256;
pub const MORSE_CAPACITY: usize = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES;

#[repr(C)]
pub struct TextFrame {
    pub input_length: u32,
    pub output_length: u32,
    pub capacity: u32,
    pub status: u32,
    pub probe: u32,
    pub target: u64,
    pub command: u32,
    pub operation: u32,
    pub work_units: u32,
    pub capability: u64,
    pub input: [u8; TEXT_CAPACITY],
    pub output: [u8; TEXT_CAPACITY],
    pub intermediate_length: u32,
    pub intermediate: [u8; 4],
    pub unit_millis: u32,
    pub morse_status: u32,
    pub morse_length: u32,
    pub morse: [u8; MORSE_CAPACITY],
    pub timer_handle: u64,
    pub count_handle: u64,
    pub timer_node: u32,
    pub timer_request: u32,
    pub timer_slot: u32,
    pub timer_generation: u32,
    pub timer_value_bytes: u32,
    pub timer_admitted_bytes: u32,
    pub timer_kind: u32,
    pub timer_decisions: u32,
    pub timer_signs: u32,
    pub timer_pending: u32,
    pub timer_status: u32,
}

const _: () = assert!(core::mem::size_of::<TextFrame>() <= 4096);
