//! Immutable external Fore custody; ownership never acknowledges delivery.
use conduit_core::{ConnectionTrack, KindId, PortId};
use std::{fmt, ops::Deref, sync::Arc};

#[derive(Debug, PartialEq, Eq)]
pub struct ExternalForeIdentity {
    pub front_port_id: PortId,
    pub track: ConnectionTrack,
    pub value_kind: KindId,
}

/// Read-only shared payload. Internal prepared storage requires unique custody to fill.
#[derive(Clone)]
pub struct ExternalForeBytes {
    storage: Arc<[u8]>,
    len: usize,
}
impl ExternalForeBytes {
    pub fn as_slice(&self) -> &[u8] {
        self
    }
    fn prepared(capacity: usize) -> Self {
        Self {
            storage: vec![0; capacity].into(),
            len: 0,
        }
    }
    fn fill(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > self.storage.len() {
            return Err("Fore output exceeded its pre-Play byte bound".into());
        }
        let storage = Arc::get_mut(&mut self.storage)
            .ok_or("Fore output prepared storage already escaped")?;
        storage[..bytes.len()].copy_from_slice(bytes);
        self.len = bytes.len();
        Ok(())
    }
}
impl From<&[u8]> for ExternalForeBytes {
    fn from(bytes: &[u8]) -> Self {
        Self {
            storage: bytes.into(),
            len: bytes.len(),
        }
    }
}
impl From<Vec<u8>> for ExternalForeBytes {
    fn from(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        Self {
            storage: bytes.into(),
            len,
        }
    }
}
impl Deref for ExternalForeBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.storage[..self.len]
    }
}
impl AsRef<[u8]> for ExternalForeBytes {
    fn as_ref(&self) -> &[u8] {
        self
    }
}
impl fmt::Debug for ExternalForeBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}
impl<T: AsRef<[u8]> + ?Sized> PartialEq<T> for ExternalForeBytes {
    fn eq(&self, other: &T) -> bool {
        self.deref() == other.as_ref()
    }
}
impl Eq for ExternalForeBytes {}
impl serde::Serialize for ExternalForeBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.deref(), serializer)
    }
}

/// Owned custody of one exact output with shared immutable identity and payload.
/// Cloning retains the same bytes without copying them or acknowledging an effect.
/// Metadata reads use the original field names through `Deref`; construction uses
/// `new`. Consumers needing independently mutable bytes explicitly call `to_vec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalForeDelivery {
    identity: Arc<ExternalForeIdentity>,
    pub sequence: u64,
    pub bytes: ExternalForeBytes,
}
impl ExternalForeDelivery {
    pub fn new(
        front_port_id: PortId,
        track: ConnectionTrack,
        value_kind: KindId,
        sequence: u64,
        bytes: impl Into<ExternalForeBytes>,
    ) -> Self {
        Self::with_identity(
            Arc::new(ExternalForeIdentity {
                front_port_id,
                track,
                value_kind,
            }),
            sequence,
            bytes.into(),
        )
    }
    pub(crate) fn with_identity(
        identity: Arc<ExternalForeIdentity>,
        sequence: u64,
        bytes: ExternalForeBytes,
    ) -> Self {
        Self {
            identity,
            sequence,
            bytes,
        }
    }
    pub(crate) fn prepared(identity: Arc<ExternalForeIdentity>, capacity: usize) -> Self {
        Self::with_identity(identity, 0, ExternalForeBytes::prepared(capacity))
    }
    pub(crate) fn fill_prepared(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), String> {
        self.bytes.fill(bytes)?;
        self.sequence = sequence;
        Ok(())
    }
}
impl Deref for ExternalForeDelivery {
    type Target = ExternalForeIdentity;
    fn deref(&self) -> &ExternalForeIdentity {
        &self.identity
    }
}

/// Success accepts the exact effect; shared custody alone has no such meaning.
pub trait ExternalForeOutputAdapter {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String>;

    /// Synchronous adapters can borrow; retaining adapters receive a cheap shared clone.
    fn deliver_flow(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.deliver(output.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> Arc<ExternalForeIdentity> {
        Arc::new(ExternalForeIdentity {
            front_port_id: "output".into(),
            track: ConnectionTrack::Payload,
            value_kind: "value/u64".into(),
        })
    }

    #[test]
    fn owned_custodies_share_exact_identity_and_payload_without_allocating() {
        let identity = identity();
        let mut report = ExternalForeDelivery::prepared(identity.clone(), 8);
        let probe = crate::allocation_probe::begin();
        report.fill_prepared(17, &[1, 2, 3]).unwrap();
        let adapter = report.clone();
        assert_eq!(probe.finish(), 0);
        assert!(Arc::ptr_eq(&report.identity, &adapter.identity));
        assert!(Arc::ptr_eq(&report.identity, &identity));
        assert!(Arc::ptr_eq(&report.bytes.storage, &adapter.bytes.storage));
        assert_eq!(report.bytes, [1, 2, 3]);
        assert_eq!(serde_json::to_string(&report.bytes).unwrap(), "[1,2,3]");
        drop(report);
        assert_eq!(adapter.sequence, 17);
        assert_eq!(adapter.bytes, [1, 2, 3]);
    }

    #[test]
    fn escaped_or_oversized_prepared_payload_refuses_without_mutation_or_copy_on_write() {
        let mut report = ExternalForeDelivery::prepared(identity(), 3);
        report.fill_prepared(4, &[1, 2]).unwrap();
        assert!(report.fill_prepared(5, &[0; 4]).is_err());
        assert_eq!(report.sequence, 4);
        assert_eq!(report.bytes, [1, 2]);
        let retained = report.clone();
        assert!(report.fill_prepared(5, &[3]).is_err());
        assert_eq!(retained, report);
        assert_eq!(report.sequence, 4);
    }

    #[test]
    fn empty_payload_and_empty_prepared_extent_preserve_the_exact_slice() {
        let mut report = ExternalForeDelivery::prepared(identity(), 0);
        report.fill_prepared(0, &[]).unwrap();
        assert_eq!(report.bytes, []);
        assert!(report.fill_prepared(1, &[1]).is_err());
        assert_eq!(report.sequence, 0);
    }
}
