//! Lossless, separately bounded transfer of a live Sign prefix.
//!
//! This is prepared storage, not an execution grant, transport acknowledgment,
//! persistence promise or invocation rearm. The caller must independently admit
//! BOTH the active log and this archive, including their simultaneous backing.
//! Default logs and mandatory retention rules are unchanged.
use crate::{
    invocation_archive::{ArchivedInvocation, ArchivedSign, InvocationArchive, InvocationEvidence},
    HostedSignLog, KernelEvent, KernelEventKind, NodeId, PortId, RemoteLifecycleIdentity,
    RequestId, SignError, SignQuery, SignRetentionGap, SignSink,
};
use core::mem::size_of;

/// An additional finite archive selection, never inferred from active-log stock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLimits {
    pub retained_events: u32,
    /// Canonical event charges PLUS exact remote-identity sidecar charges.
    pub retained_bytes: u64,
    pub archive_backing_bytes: usize,
    /// Complete wrapper, active allocations and borrowed transfer token overlap.
    /// This is owned backing accounting, not a compiler stack-depth bound.
    pub transfer_overlap_bytes: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferRefusal {
    Bounds,
    Empty,
    Sequence,
    ArchiveCapacity,
    ObserverBacking,
}
/// Refused preparation returns the exact original owner, including its Signs.
pub struct RejectedPreparation {
    pub reason: TransferRefusal,
    active: HostedSignLog,
}
impl RejectedPreparation {
    pub fn into_active(self) -> HostedSignLog {
        self.active
    }
}
impl core::fmt::Debug for RejectedPreparation {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        output
            .debug_struct("RejectedPreparation")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferredPrefix {
    pub first_sequence: u32,
    pub next_sequence: u32,
    pub events: u32,
}

/// The same active log remains owned here across every prefix transfer. There
/// is no API to substitute another log using copied identities or counters.
pub struct LiveSignLog<const EVENTS: usize, const RUNS: usize, const STREAMS: usize> {
    active: HostedSignLog,
    // Reuse exact template/affine encoding for opaque trace SEGMENTS. The
    // internal metadata's decisions field carries sequence span; these are
    // not claims about completed kernel invocations or a prepared-session rearm.
    archive: InvocationArchive<EVENTS, RUNS, STREAMS>,
    limits: ArchiveLimits,
    next_archived_sequence: u32,
    archived_events: u32,
    archived_bytes: u64,
    archived_gap: Option<SignRetentionGap>,
    last_archived: Option<ArchivedSign>,
    archived_kinds: u64,
}
impl<const E: usize, const R: usize, const S: usize> LiveSignLog<E, R, S> {
    pub const fn required_archive_backing_bytes() -> usize {
        size_of::<InvocationArchive<E, R, S>>()
    }
    pub fn required_transfer_overlap_bytes(active: &HostedSignLog) -> Option<usize> {
        size_of::<Self>()
            .checked_add(active.reserved_storage_bytes())?
            .checked_add(size_of::<PreparedPrefixTransfer<'_, E, R, S>>())
    }
    /// No allocation, grant or effects. The supplied active owner is moved once.
    pub fn prepare(
        active: HostedSignLog,
        limits: ArchiveLimits,
    ) -> Result<Self, RejectedPreparation> {
        if limits.retained_events == 0
            || limits.retained_bytes == 0
            || Self::required_archive_backing_bytes() > limits.archive_backing_bytes
            || Self::required_transfer_overlap_bytes(&active)
                .is_none_or(|n| n > limits.transfer_overlap_bytes)
        {
            return Err(RejectedPreparation {
                reason: TransferRefusal::Bounds,
                active,
            });
        }
        // Its external allocation is deliberately not guessed by this owner.
        if active.debug_observer.is_some() {
            return Err(RejectedPreparation {
                reason: TransferRefusal::ObserverBacking,
                active,
            });
        }
        Ok(Self {
            active,
            archive: InvocationArchive::new(),
            limits,
            next_archived_sequence: 0,
            archived_events: 0,
            archived_bytes: 0,
            archived_gap: None,
            last_archived: None,
            archived_kinds: 0,
        })
    }
    pub fn archived_event_count(&self) -> u32 {
        self.archived_events
    }
    pub fn archived_canonical_bytes(&self) -> u64 {
        self.archived_bytes
    }
    pub fn owned_backing_bytes(&self) -> Option<usize> {
        size_of::<Self>().checked_add(self.active.reserved_storage_bytes())
    }
    /// Read-only admission followed by an exclusive borrow of the SAME owner.
    /// Dropping the token changes neither active nor archived evidence.
    pub fn prepare_prefix_transfer(
        &mut self,
    ) -> Result<PreparedPrefixTransfer<'_, E, R, S>, TransferRefusal> {
        let events = u32::from(self.active.len);
        if events == 0 {
            return Err(TransferRefusal::Empty);
        }
        let first = self.next_archived_sequence;
        let next = self.active.next_sequence;
        let span = next
            .checked_sub(first)
            .filter(|n| *n > 0)
            .ok_or(TransferRefusal::Sequence)?;
        let mut prior = None;
        for event in self.active.events() {
            if event.sequence < first
                || event.sequence >= next
                || prior.is_some_and(|old| old >= event.sequence)
            {
                return Err(TransferRefusal::Sequence);
            }
            prior = Some(event.sequence);
        }
        if self.active.retention_gap.is_some_and(|gap| {
            gap.first_sequence < first
                || gap.last_sequence >= next
                || gap.first_sequence > gap.last_sequence
        }) {
            return Err(TransferRefusal::Sequence);
        }
        let retained_events = self
            .archived_events
            .checked_add(events)
            .filter(|n| *n <= self.limits.retained_events)
            .ok_or(TransferRefusal::Bounds)?;
        let charge = u64::from(events)
            .checked_mul(size_of::<KernelEvent>() as u64)
            .and_then(|n| {
                n.checked_add(
                    u64::from(self.active.remote_len)
                        .checked_mul(size_of::<crate::RemoteLifecycleSign>() as u64)?,
                )
            })
            .ok_or(TransferRefusal::Bounds)?;
        let retained_bytes = self
            .archived_bytes
            .checked_add(charge)
            .filter(|n| *n <= self.limits.retained_bytes)
            .ok_or(TransferRefusal::Bounds)?;
        let gap = merge_gap(self.archived_gap, self.active.retention_gap)?;
        let prefix = NormalizedPrefix {
            active: &self.active,
            origin: first,
        };
        self.archive
            .can_append(&prefix, span, 0)
            .map_err(|_| TransferRefusal::ArchiveCapacity)?;
        Ok(PreparedPrefixTransfer {
            owner: self,
            first,
            next,
            span,
            events,
            retained_events,
            retained_bytes,
            gap,
        })
    }
    pub fn events_with_remote(&self) -> Entries<'_, E, R, S> {
        Entries {
            owner: self,
            next_segment: 0,
            current: None,
            current_origin: 0,
            next_origin: 0,
            active_index: 0,
            remaining: u64::from(self.archived_events) + u64::from(self.active.len),
        }
    }
    /// `last()` is bounded by active stock and does not scan archived history.
    pub fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.events_with_remote().map(|entry| entry.event)
    }
}

/// Opaque affine loan: all fallible checks precede archive publication or reuse.
pub struct PreparedPrefixTransfer<'a, const E: usize, const R: usize, const S: usize> {
    owner: &'a mut LiveSignLog<E, R, S>,
    first: u32,
    next: u32,
    span: u32,
    events: u32,
    retained_events: u32,
    retained_bytes: u64,
    gap: Option<SignRetentionGap>,
}
impl<const E: usize, const R: usize, const S: usize> PreparedPrefixTransfer<'_, E, R, S> {
    pub fn commit(self) -> TransferredPrefix {
        let owner = self.owner;
        let prefix = NormalizedPrefix {
            active: &owner.active,
            origin: self.first,
        };
        // The exclusive token prevents every state change between preflight and
        // this identical exact append. No user callback or allocation intervenes.
        owner
            .archive
            .append(&prefix, self.span, 0)
            .expect("same exclusively borrowed preflighted prefix");
        owner.last_archived = owner.active.events().last().map(|event| ArchivedSign {
            remote: owner.active.remote_identity(event.sequence),
            event,
        });
        for event in owner.active.events() {
            if let Some(bit) = 1_u64.checked_shl(event.kind as u32) {
                owner.archived_kinds |= bit;
            }
        }
        owner.archived_events = self.retained_events;
        owner.archived_bytes = self.retained_bytes;
        owner.archived_gap = self.gap;
        owner.next_archived_sequence = self.next;
        // Ownership has moved to the archive. This is NOT mandatory eviction,
        // a new invocation, a transport ACK, or any reset of the sequence clock.
        owner.active.entries.fill(None);
        owner.active.remote_entries.fill(None);
        owner.active.len = 0;
        owner.active.retained_items = 0;
        owner.active.used_bytes = 0;
        owner.active.remote_len = 0;
        owner.active.remote_used_bytes = 0;
        owner.active.retention_gap = None;
        TransferredPrefix {
            first_sequence: self.first,
            next_sequence: self.next,
            events: self.events,
        }
    }
}

struct NormalizedPrefix<'a> {
    active: &'a HostedSignLog,
    origin: u32,
}
impl SignQuery for NormalizedPrefix<'_> {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.active.contains_kind(kind)
    }
    fn remote_identity(&self, sequence: u32) -> Option<RemoteLifecycleIdentity> {
        self.active
            .remote_identity(sequence.checked_add(self.origin)?)
    }
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        self.active.retention_gap.map(|gap| SignRetentionGap {
            first_sequence: gap.first_sequence - self.origin,
            last_sequence: gap.last_sequence - self.origin,
            entries: gap.entries,
        })
    }
}
impl InvocationEvidence for NormalizedPrefix<'_> {
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        self.active.events().map(|mut event| {
            event.sequence -= self.origin;
            event
        })
    }
}
fn merge_gap(
    a: Option<SignRetentionGap>,
    b: Option<SignRetentionGap>,
) -> Result<Option<SignRetentionGap>, TransferRefusal> {
    Ok(match (a, b) {
        (None, gap) | (gap, None) => gap,
        (Some(a), Some(b)) => Some(SignRetentionGap {
            first_sequence: a.first_sequence.min(b.first_sequence),
            last_sequence: a.last_sequence.max(b.last_sequence),
            entries: a
                .entries
                .checked_add(b.entries)
                .ok_or(TransferRefusal::Sequence)?,
        }),
    })
}

pub struct Entries<'a, const E: usize, const R: usize, const S: usize> {
    owner: &'a LiveSignLog<E, R, S>,
    next_segment: u32,
    current: Option<ArchivedInvocation<'a, S>>,
    current_origin: u32,
    next_origin: u32,
    active_index: usize,
    remaining: u64,
}
impl<const E: usize, const R: usize, const S: usize> Iterator for Entries<'_, E, R, S> {
    type Item = ArchivedSign;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.current.as_mut().and_then(Iterator::next) {
                let mut entry = entry;
                entry.event.sequence = entry
                    .event
                    .sequence
                    .checked_add(self.current_origin)
                    .expect("checked archived sequence span");
                self.remaining -= 1;
                return Some(entry);
            }
            if let Some(segment) = self.owner.archive.invocation(self.next_segment) {
                self.next_segment += 1;
                self.current_origin = self.next_origin;
                self.next_origin = self
                    .next_origin
                    .checked_add(segment.metadata.decisions)
                    .expect("checked consecutive sequence spans");
                self.current = Some(segment);
                continue;
            }
            while self.active_index < usize::from(self.owner.active.len) {
                let index = self.active_index;
                self.active_index += 1;
                if let Some(event) = self.owner.active.entries[index].map(KernelEvent::from) {
                    self.remaining -= 1;
                    return Some(ArchivedSign {
                        remote: self.owner.active.remote_identity(event.sequence),
                        event,
                    });
                }
            }
            return None;
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match usize::try_from(self.remaining) {
            Ok(n) => (n, Some(n)),
            Err(_) => (usize::MAX, None),
        }
    }
    fn last(self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        if self.active_index < usize::from(self.owner.active.len) {
            return self.owner.active.events().last().map(|event| ArchivedSign {
                remote: self.owner.active.remote_identity(event.sequence),
                event,
            });
        }
        self.owner.last_archived
    }
}
impl<const E: usize, const R: usize, const S: usize> SignQuery for LiveSignLog<E, R, S> {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.active.contains_kind(kind)
            || match 1_u64.checked_shl(kind as u32) {
                Some(bit) => self.archived_kinds & bit != 0,
                None => self.events().any(|event| event.kind == kind),
            }
    }
    fn remote_identity(&self, sequence: u32) -> Option<RemoteLifecycleIdentity> {
        if sequence >= self.next_archived_sequence {
            self.active.remote_identity(sequence)
        } else {
            self.events_with_remote()
                .find(|entry| entry.event.sequence == sequence)?
                .remote
        }
    }
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        // Publication sequence admission bounds each disjoint gap count by the
        // same u32 global clock, so a real combined gap cannot overflow here.
        merge_gap(self.archived_gap, self.active.retention_gap)
            .expect("disjoint gaps within original global sequence clock")
    }
}
impl<const E: usize, const R: usize, const S: usize> InvocationEvidence for LiveSignLog<E, R, S> {
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        self.events()
    }
}
impl<const E: usize, const R: usize, const S: usize> SignSink for LiveSignLog<E, R, S> {
    // The original active window keeps its EXACT quota. Archive custody has
    // separate explicit limits; no method silently rotates or enlarges this log.
    fn item_capacity(&self) -> u16 {
        self.active.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.active.byte_capacity()
    }
    fn len(&self) -> u16 {
        self.active.len()
    }
    fn used_bytes(&self) -> u32 {
        self.active.used_bytes()
    }
    fn ensure_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.active.ensure_capacity(additional)
    }
    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        self.active.record(node, port, request, kind)
    }
    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        self.active.record_remote(node, port, kind, remote)
    }
    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.active.ensure_remote_capacity(additional)
    }
    fn observe_debug(&mut self, event: crate::debug_observation::DebugRuntimeEvent<'_>) {
        self.active.observe_debug(event);
    }
    fn requires_canonical_debug_values(&self) -> bool {
        self.active.requires_canonical_debug_values()
    }
}

#[cfg(test)]
mod tests;
