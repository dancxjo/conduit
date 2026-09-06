//! Bounded canonical values derived by an operation during one kernel step.

use super::SchedulerError;
use crate::{PortId, StorageError, ValueRef, ValueStorage};

pub const MAXIMUM_DERIVED_VALUE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalValue {
    len: u8,
    bytes: [u8; MAXIMUM_DERIVED_VALUE_BYTES],
}

impl CanonicalValue {
    /// Maximum canonical bytes carried by one derived emission.
    pub const MAXIMUM_BYTES: usize = MAXIMUM_DERIVED_VALUE_BYTES;

    pub fn new(bytes: &[u8]) -> Result<Self, StorageError> {
        let len = u8::try_from(bytes.len()).map_err(|_| StorageError::ValueTooLarge)?;
        if bytes.len() > MAXIMUM_DERIVED_VALUE_BYTES {
            return Err(StorageError::ValueTooLarge);
        }
        let mut value = Self {
            len,
            bytes: [0; MAXIMUM_DERIVED_VALUE_BYTES],
        };
        value.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(value)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

pub(super) fn materialize<S: ValueStorage, const PORTS: usize>(
    values: &mut S,
    canonical: Option<(PortId, CanonicalValue)>,
    outputs: &mut [Option<ValueRef>; PORTS],
) -> Result<Option<ValueRef>, SchedulerError> {
    let Some((port, canonical)) = canonical else {
        return Ok(None);
    };
    let output = outputs
        .get_mut(usize::from(port.0))
        .ok_or(SchedulerError::InvalidPortAccess)?;
    let value = values.store(canonical.as_slice())?;
    *output = Some(value);
    Ok(Some(value))
}
