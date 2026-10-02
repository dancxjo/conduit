//! Shared error and native codec helpers for navigation values.

use alloc::vec::Vec;
use conduit_core::StructuredInfoRefusal;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

use crate::NavigationRefusal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationCodecError {
    Malformed,
    InexactQuantity,
    Structured(StructuredInfoRefusal),
    Navigation(NavigationRefusal),
    Robotics,
}

impl From<StructuredInfoRefusal> for NavigationCodecError {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

impl From<NavigationRefusal> for NavigationCodecError {
    fn from(value: NavigationRefusal) -> Self {
        Self::Navigation(value)
    }
}

pub(crate) fn encode_native<T: NativeRustBinding + Clone>(
    value: &T,
) -> Result<Vec<u8>, NavigationCodecError> {
    value
        .clone()
        .encode()
        .map_err(|_: NativeBindingRefusal| NavigationCodecError::Malformed)
}

pub(crate) fn decode_native<T: NativeRustBinding>(
    encoded: &[u8],
) -> Result<T, NavigationCodecError> {
    T::decode(encoded).map_err(|_: NativeBindingRefusal| NavigationCodecError::Malformed)
}
