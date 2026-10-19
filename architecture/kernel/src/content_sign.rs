//! Bounded content bindings beside the existing authoritative routing Signs.
//! Digest choice, input-byte ceilings and hashing work belong to the admitted
//! profile. This module adds no crypto dependency or semantic value ontology.
use crate::{
    CordId, FixedSignLog, KernelEvent, KernelEventKind, NodeId, PortId, RemoteLifecycleIdentity,
    RequestId, SignError, SignQuery, SignRetentionGap, SignSink,
};

/// An infallible, bounded, allocation-free digest implementation chosen before
/// Play. Its algorithm identity and work/storage bounds must be retained by the
/// owning profile; these bytes alone do not identify a hash algorithm.
pub trait ContentDigestProvider {
    fn digest(&self, bytes: &[u8]) -> [u8; 32];
}
/// Logs using the shared oldest-transient eviction rule and exact monotonic
/// retention gaps. A ValueRouted event at/before the gap's last sequence is gone.
pub trait OrderedTransientRetention: SignSink + SignQuery {}
impl<const N: usize> OrderedTransientRetention for FixedSignLog<N> {}
#[cfg(feature = "alloc")]
impl OrderedTransientRetention for crate::HostedSignLog {}

/// Join to the retained KernelEvent by exact sequence, then to the owning
/// execution's node/port bindings. Not a free-standing execution attestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentSignBinding {
    pub event_sequence: u32,
    pub cord: CordId,
    pub byte_length: u32,
    pub digest: [u8; 32],
}
pub fn content_sign_storage_bytes(items: u16) -> Option<u32> {
    usize::from(items)
        .checked_mul(core::mem::size_of::<Option<ContentSignBinding>>())
        .and_then(|n| u32::try_from(n).ok())
}
/// Explicit optional profile. Its fixed binding array can hold every retained
/// event in the underlying log, so content pressure cannot cause a late commit
/// refusal. Bindings expire with their actual routing Sign, never independently.
pub type ContentSignLog<S, D, const N: usize> =
    BoundedContentSignLog<S, D, [Option<ContentSignBinding>; N]>;
#[cfg(feature = "alloc")]
pub type HostedContentSignLog<S, D> =
    BoundedContentSignLog<S, D, alloc::vec::Vec<Option<ContentSignBinding>>>;

pub struct BoundedContentSignLog<S, D, B> {
    inner: S,
    provider: D,
    entries: B,
    len: u16,
    pruned_through: Option<u32>,
}
impl<S: OrderedTransientRetention, D: ContentDigestProvider, const N: usize>
    ContentSignLog<S, D, N>
{
    pub fn new(inner: S, provider: D, content_byte_budget: u32) -> Result<Self, SignError> {
        let items = u16::try_from(N).map_err(|_| SignError::InvalidBudget)?;
        let bytes = content_sign_storage_bytes(items).ok_or(SignError::InvalidBudget)?;
        if N == 0 || inner.item_capacity() > items || content_byte_budget != bytes {
            return Err(SignError::InvalidBudget);
        }
        Ok(Self {
            inner,
            provider,
            entries: [None; N],
            len: 0,
            pruned_through: None,
        })
    }
}
#[cfg(feature = "alloc")]
impl<S: OrderedTransientRetention, D: ContentDigestProvider> HostedContentSignLog<S, D> {
    /// One exact, pre-Play heap allocation for the admitted retained-event
    /// ceiling. Execution never reserves, resizes or grows this storage.
    pub fn new(inner: S, provider: D, content_byte_budget: u32) -> Result<Self, SignError> {
        let items = inner.item_capacity();
        let bytes = content_sign_storage_bytes(items).ok_or(SignError::InvalidBudget)?;
        if items == 0 || content_byte_budget != bytes {
            return Err(SignError::InvalidBudget);
        }
        let entries = alloc::vec![None;usize::from(items)];
        if entries.capacity() != usize::from(items) {
            return Err(SignError::InvalidBudget);
        }
        Ok(Self {
            inner,
            provider,
            entries,
            len: 0,
            pruned_through: None,
        })
    }
    pub fn content_allocation_capacity(&self) -> usize {
        self.entries.capacity()
    }
}
impl<S: OrderedTransientRetention, D: ContentDigestProvider, B> BoundedContentSignLog<S, D, B>
where
    B: AsRef<[Option<ContentSignBinding>]> + AsMut<[Option<ContentSignBinding>]>,
{
    pub const fn inner(&self) -> &S {
        &self.inner
    }
    pub const fn provider(&self) -> &D {
        &self.provider
    }
    /// Complete inline container, including S and D. Their separately owned
    /// heap storage, allocator metadata, input bytes and stack are excluded.
    pub const fn container_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
    }
    pub fn content_capacity_bytes(&self) -> u32 {
        content_sign_storage_bytes(self.entries.as_ref().len() as u16)
            .expect("constructor checked fixed capacity")
    }
    pub fn bindings(&self) -> impl Iterator<Item = ContentSignBinding> + '_ {
        self.entries.as_ref()[..usize::from(self.len)]
            .iter()
            .copied()
            .flatten()
    }
    pub fn binding(&self, event_sequence: u32) -> Option<ContentSignBinding> {
        self.bindings().find(|b| b.event_sequence == event_sequence)
    }
    fn prune(&mut self) {
        let Some(last) = self.inner.retention_gap().map(|g| g.last_sequence) else {
            return;
        };
        if self.pruned_through == Some(last) {
            return;
        }
        let len = usize::from(self.len);
        let expired = self.entries.as_ref()[..len]
            .iter()
            .take_while(|row| row.is_some_and(|b| b.event_sequence <= last))
            .count();
        self.entries.as_mut()[..len].rotate_left(expired);
        self.entries.as_mut()[len - expired..len].fill(None);
        self.len -= expired as u16;
        self.pruned_through = Some(last);
    }
}
impl<S: OrderedTransientRetention, D: ContentDigestProvider, B> SignSink
    for BoundedContentSignLog<S, D, B>
where
    B: AsRef<[Option<ContentSignBinding>]> + AsMut<[Option<ContentSignBinding>]>,
{
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn len(&self) -> u16 {
        self.inner.len()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn ensure_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.inner.ensure_capacity(additional)
    }
    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        let event = self.inner.record(node, port, request, kind)?;
        self.prune();
        Ok(event)
    }
    fn record_value(
        &mut self,
        node: NodeId,
        port: PortId,
        cord: CordId,
        bytes: &[u8],
    ) -> Result<KernelEvent, SignError> {
        let byte_length = u32::try_from(bytes.len()).map_err(|_| SignError::InvalidBudget)?;
        self.inner.ensure_capacity(1)?;
        let digest = self.provider.digest(bytes);
        let event = self
            .inner
            .record(node, Some(port), None, KernelEventKind::ValueRouted)?;
        self.prune();
        // Every retained binding owns a distinct retained event. Constructor
        // admission makes N >= all retained events; this slot must be free.
        self.entries.as_mut()[usize::from(self.len)] = Some(ContentSignBinding {
            event_sequence: event.sequence,
            cord,
            byte_length,
            digest,
        });
        self.len += 1;
        Ok(event)
    }
    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        let event = self.inner.record_remote(node, port, kind, remote)?;
        self.prune();
        Ok(event)
    }
    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.inner.ensure_remote_capacity(additional)
    }
    fn observe_debug(&mut self, event: crate::debug_observation::DebugRuntimeEvent<'_>) {
        self.inner.observe_debug(event)
    }
}
impl<S: OrderedTransientRetention, D: ContentDigestProvider, B> SignQuery
    for BoundedContentSignLog<S, D, B>
where
    B: AsRef<[Option<ContentSignBinding>]> + AsMut<[Option<ContentSignBinding>]>,
{
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.inner.contains_kind(kind)
    }
    fn remote_identity(&self, seq: u32) -> Option<RemoteLifecycleIdentity> {
        self.inner.remote_identity(seq)
    }
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        self.inner.retention_gap()
    }
}
