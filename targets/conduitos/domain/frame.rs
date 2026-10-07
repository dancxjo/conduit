//! One bounded copy window shared by Root and the separately linked image.

pub const TEXT_CAPACITY: usize = 256;

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
}
