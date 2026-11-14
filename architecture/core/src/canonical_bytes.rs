//! One canonical encoding can feed stored bytes or the streaming fingerprint.
//! The sink changes representation costs, never field order or identity bytes.
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub(crate) trait CanonicalSink {
    fn scope_string(&mut self, _field: FragmentScopeField<'_>, value: &str)
    where
        Self: Sized,
    {
        crate::push_string(self, value);
    }
    fn scope_u64(&mut self, _field: FragmentScopeField<'_>, value: u64)
    where
        Self: Sized,
    {
        crate::push_u64(self, value);
    }
    fn push(&mut self, byte: u8);
    fn extend_from_slice(&mut self, bytes: &[u8]);
}
impl CanonicalSink for Vec<u8> {
    fn push(&mut self, byte: u8) {
        Vec::push(self, byte);
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        Vec::extend_from_slice(self, bytes);
    }
}
pub(crate) struct CanonicalHash(Sha256);
impl CanonicalHash {
    pub(crate) fn new() -> Self {
        Self(Sha256::new())
    }
    pub(crate) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
impl CanonicalSink for CanonicalHash {
    fn push(&mut self, byte: u8) {
        self.0.update([byte]);
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

/// Semantic identity fields in the existing fragment serialization. These
/// distinguish substitutions from identical text occurring in static Source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentScopeRole {
    Host,
    Boot,
    OfferGeneration,
    PlacementId,
    ProviderInstance,
    ProviderGeneration,
    AuthorityGrant,
    AuthorityHost,
    AuthorityBoot,
    ResourcePool,
    ContentOwnerHost,
    ContentOwnerBoot,
    PoolReference,
}
#[derive(Clone, Copy)]
pub(crate) enum FragmentScopeField<'a> {
    Header(FragmentScopeRole),
    Placement(&'a crate::PlacementId, FragmentScopeRole, usize),
}
