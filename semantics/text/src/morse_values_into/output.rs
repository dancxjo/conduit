//! Shared bounded storage for canonical Morse encoders.
use crate::MorseError;
use alloc::vec::Vec;
use core::ops::{Deref, DerefMut};

pub(super) enum Output<'a> {
    Vec(&'a mut Vec<u8>),
    Slice { bytes: &'a mut [u8], length: usize },
}
impl<'a> Output<'a> {
    pub fn vec(bytes: &'a mut Vec<u8>) -> Self {
        Self::Vec(bytes)
    }
    pub fn slice(bytes: &'a mut [u8]) -> Self {
        Self::Slice { bytes, length: 0 }
    }
    pub fn capacity(&self) -> usize {
        match self {
            Self::Vec(bytes) => bytes.capacity(),
            Self::Slice { bytes, .. } => bytes.len(),
        }
    }
    pub fn clear(&mut self) {
        match self {
            Self::Vec(bytes) => bytes.clear(),
            Self::Slice { length, .. } => *length = 0,
        }
    }
    pub fn push(&mut self, byte: u8) -> Result<(), MorseError> {
        self.extend_from_slice(&[byte])
    }
    pub fn extend_from_slice(&mut self, input: &[u8]) -> Result<(), MorseError> {
        let end = self
            .len()
            .checked_add(input.len())
            .filter(|end| *end <= self.capacity())
            .ok_or(MorseError::OutputCapacity)?;
        match self {
            Self::Vec(bytes) => bytes.extend_from_slice(input),
            Self::Slice { bytes, length } => {
                bytes[*length..end].copy_from_slice(input);
                *length = end;
            }
        }
        Ok(())
    }
}
impl Deref for Output<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Vec(bytes) => bytes.as_slice(),
            Self::Slice { bytes, length } => &bytes[..*length],
        }
    }
}
impl DerefMut for Output<'_> {
    fn deref_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Vec(bytes) => bytes.as_mut_slice(),
            Self::Slice { bytes, length } => &mut bytes[..*length],
        }
    }
}
