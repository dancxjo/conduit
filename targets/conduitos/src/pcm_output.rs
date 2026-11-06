//! Finite native PCM mechanism beneath ordinary audible Host Calls.
use crate::machine::BaseError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcmProvider {
    pub base_id: &'static str,
    pub provider_generation: u64,
    pub sample_rate: u32,
    pub maximum_frames: u32,
}

pub trait PcmOutput {
    fn provider(&self) -> PcmProvider;
    /// Submit one prepared signed-16 mono buffer. Completion means the finite
    /// hardware operation finished; it does not attest that a speaker was heard.
    fn play(&mut self, samples: &[i16]) -> Result<(), BaseError>;
}
