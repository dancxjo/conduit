//! Allocator-free semantics for an optional best-effort evictable blob cache.
//!
//! Handles are provider- and run-scoped references, never authority. The
//! caller must independently possess the exact resource grant pinned by its
//! execution plan.

use sha2::{Digest as _, Sha256};

/// Maximum entries in the deterministic reference profile.
pub const CACHE_MAX_ENTRIES: usize = 4;
/// Maximum bytes retained for one blob in the reference profile.
pub const CACHE_MAX_BLOB_BYTES: usize = 32_768;

/// Exact persistence requested from a storage provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CachePersistence {
    /// The value may be evicted or disappear with the provider.
    Evictable,
    /// The value must survive a durable commit boundary.
    Durable,
}

/// Sensitivity accepted by a cache resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CacheSensitivity {
    Public,
    Restricted,
    Secret,
}

/// One caller requirement checked before provider selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheRequirement {
    pub persistence: CachePersistence,
    pub maximum_blob_bytes: usize,
    pub sensitivity: CacheSensitivity,
}

/// Content identity checked before a cache hit yields bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlobIdentity {
    pub digest: [u8; 32],
    pub bytes: usize,
}

impl BlobIdentity {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            digest: Sha256::digest(bytes).into(),
            bytes: bytes.len(),
        }
    }
}

/// Opaque non-authoritative cache reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheHandle {
    pub provider_epoch: u64,
    pub run_epoch: u64,
    pub slot: u16,
    pub generation: u64,
    pub identity: BlobIdentity,
}

/// One exact bounded put request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PutRequest {
    pub run_epoch: u64,
    pub now_tick: u64,
    pub retention_ticks: u64,
    pub maximum_blob_bytes: usize,
    pub sensitivity: CacheSensitivity,
}

/// Put outcome, including any deterministic eviction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PutResult {
    pub handle: CacheHandle,
    pub evicted: Option<BlobIdentity>,
    pub expires_at_tick: u64,
}

/// One exact bounded get request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GetRequest {
    pub run_epoch: u64,
    pub now_tick: u64,
    pub maximum_blob_bytes: usize,
    pub handle: CacheHandle,
}

/// Explicit hit/miss result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GetOutcome {
    Hit,
    Miss,
    Evicted,
    Expired,
}

/// Get result metadata. Bytes are copied into caller-owned bounded storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GetResult {
    pub outcome: GetOutcome,
    pub identity: BlobIdentity,
    pub bytes_read: usize,
}

/// Explicit removal result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoveOutcome {
    Removed,
    Missing,
}

/// Stable cache failure reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheError {
    InvalidBound,
    DurableUnsupported,
    SensitivityRefused,
    Oversized,
    CapacityUnavailable,
    ProviderUnavailable,
    WrongProvider,
    WrongRun,
    InvalidHandle,
    DigestMismatch,
    Cancelled,
}

impl CacheError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidBound => "CND-CACHE-001",
            Self::DurableUnsupported => "CND-CACHE-002",
            Self::SensitivityRefused => "CND-CACHE-003",
            Self::Oversized => "CND-CACHE-004",
            Self::CapacityUnavailable => "CND-CACHE-005",
            Self::ProviderUnavailable => "CND-CACHE-006",
            Self::WrongProvider => "CND-CACHE-007",
            Self::WrongRun => "CND-CACHE-008",
            Self::InvalidHandle => "CND-CACHE-009",
            Self::DigestMismatch => "CND-CACHE-010",
            Self::Cancelled => "CND-CACHE-011",
        }
    }
}

#[derive(Clone, Copy)]
struct CacheSlot<const BYTES: usize> {
    bytes: [u8; BYTES],
    length: usize,
    identity: BlobIdentity,
    generation: u64,
    expires_at_tick: u64,
    inserted_at_tick: u64,
    sensitivity: CacheSensitivity,
    occupied: bool,
    evicted_generation: u64,
}

impl<const BYTES: usize> CacheSlot<BYTES> {
    const EMPTY: Self = Self {
        bytes: [0; BYTES],
        length: 0,
        identity: BlobIdentity {
            digest: [0; 32],
            bytes: 0,
        },
        generation: 0,
        expires_at_tick: 0,
        inserted_at_tick: 0,
        sensitivity: CacheSensitivity::Public,
        occupied: false,
        evicted_generation: 0,
    };
}

/// Fixed-capacity deterministic cache with FIFO eviction.
pub struct CacheStore<const ENTRIES: usize, const BYTES: usize> {
    provider_epoch: u64,
    maximum_total_bytes: usize,
    maximum_retention_ticks: u64,
    accepted_sensitivity: CacheSensitivity,
    slots: [CacheSlot<BYTES>; ENTRIES],
    retained_bytes: usize,
    next_generation: u64,
    available: bool,
    cancelled: bool,
}

impl<const ENTRIES: usize, const BYTES: usize> CacheStore<ENTRIES, BYTES> {
    #[must_use]
    pub const fn new(
        provider_epoch: u64,
        maximum_total_bytes: usize,
        maximum_retention_ticks: u64,
        accepted_sensitivity: CacheSensitivity,
    ) -> Self {
        Self {
            provider_epoch,
            maximum_total_bytes,
            maximum_retention_ticks,
            accepted_sensitivity,
            slots: [CacheSlot::EMPTY; ENTRIES],
            retained_bytes: 0,
            next_generation: 1,
            available: true,
            cancelled: false,
        }
    }

    #[must_use]
    pub fn satisfies(&self, requirement: CacheRequirement) -> bool {
        requirement.persistence == CachePersistence::Evictable
            && requirement.maximum_blob_bytes > 0
            && requirement.maximum_blob_bytes <= BYTES
            && requirement.maximum_blob_bytes <= self.maximum_total_bytes
            && requirement.sensitivity as u8 <= self.accepted_sensitivity as u8
    }

    pub const fn set_available(&mut self, available: bool) {
        self.available = available;
    }

    pub const fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn put(&mut self, request: PutRequest, bytes: &[u8]) -> Result<PutResult, CacheError> {
        self.check_operation()?;
        if request.maximum_blob_bytes == 0
            || request.maximum_blob_bytes > BYTES
            || request.retention_ticks == 0
            || request.retention_ticks > self.maximum_retention_ticks
        {
            return Err(CacheError::InvalidBound);
        }
        if request.sensitivity > self.accepted_sensitivity {
            return Err(CacheError::SensitivityRefused);
        }
        if bytes.len() > request.maximum_blob_bytes || bytes.len() > BYTES {
            return Err(CacheError::Oversized);
        }
        if bytes.len() > self.maximum_total_bytes {
            return Err(CacheError::CapacityUnavailable);
        }
        let expires_at_tick = request
            .now_tick
            .checked_add(request.retention_ticks)
            .ok_or(CacheError::InvalidBound)?;
        self.expire(request.now_tick);
        let mut evicted = None;
        let index = if let Some(index) = self.slots.iter().position(|slot| !slot.occupied) {
            index
        } else {
            let index = self
                .slots
                .iter()
                .enumerate()
                .min_by_key(|(_, slot)| (slot.inserted_at_tick, slot.generation))
                .map(|(index, _)| index)
                .ok_or(CacheError::CapacityUnavailable)?;
            evicted = Some(self.slots[index].identity);
            self.retained_bytes -= self.slots[index].length;
            self.slots[index].evicted_generation = self.slots[index].generation;
            index
        };
        while self.retained_bytes + bytes.len() > self.maximum_total_bytes {
            let Some(victim) = self
                .slots
                .iter()
                .enumerate()
                .filter(|(candidate, slot)| *candidate != index && slot.occupied)
                .min_by_key(|(_, slot)| (slot.inserted_at_tick, slot.generation))
                .map(|(candidate, _)| candidate)
            else {
                return Err(CacheError::CapacityUnavailable);
            };
            evicted = Some(self.slots[victim].identity);
            self.retained_bytes -= self.slots[victim].length;
            self.slots[victim].evicted_generation = self.slots[victim].generation;
            self.slots[victim].occupied = false;
        }
        let identity = BlobIdentity::from_bytes(bytes);
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(CacheError::InvalidBound)?;
        let slot = &mut self.slots[index];
        slot.bytes[..bytes.len()].copy_from_slice(bytes);
        slot.length = bytes.len();
        slot.identity = identity;
        slot.generation = generation;
        slot.expires_at_tick = expires_at_tick;
        slot.inserted_at_tick = request.now_tick;
        slot.sensitivity = request.sensitivity;
        slot.occupied = true;
        self.retained_bytes += bytes.len();
        Ok(PutResult {
            handle: CacheHandle {
                provider_epoch: self.provider_epoch,
                run_epoch: request.run_epoch,
                slot: u16::try_from(index).map_err(|_| CacheError::CapacityUnavailable)?,
                generation,
                identity,
            },
            evicted,
            expires_at_tick,
        })
    }

    pub fn get(&mut self, request: GetRequest, output: &mut [u8]) -> Result<GetResult, CacheError> {
        self.check_operation()?;
        self.check_handle(request.run_epoch, request.handle)?;
        if request.maximum_blob_bytes == 0 || output.len() < request.maximum_blob_bytes {
            return Err(CacheError::InvalidBound);
        }
        let index = usize::from(request.handle.slot);
        let slot = self.slots.get_mut(index).ok_or(CacheError::InvalidHandle)?;
        if slot.evicted_generation == request.handle.generation {
            return Ok(GetResult {
                outcome: GetOutcome::Evicted,
                identity: request.handle.identity,
                bytes_read: 0,
            });
        }
        if !slot.occupied {
            return Ok(GetResult {
                outcome: GetOutcome::Miss,
                identity: request.handle.identity,
                bytes_read: 0,
            });
        }
        if slot.generation != request.handle.generation || slot.identity != request.handle.identity
        {
            return Ok(GetResult {
                outcome: GetOutcome::Miss,
                identity: request.handle.identity,
                bytes_read: 0,
            });
        }
        if request.now_tick >= slot.expires_at_tick {
            self.retained_bytes -= slot.length;
            slot.occupied = false;
            return Ok(GetResult {
                outcome: GetOutcome::Expired,
                identity: request.handle.identity,
                bytes_read: 0,
            });
        }
        if slot.length > request.maximum_blob_bytes {
            return Err(CacheError::Oversized);
        }
        let observed = BlobIdentity::from_bytes(&slot.bytes[..slot.length]);
        if observed != slot.identity {
            return Err(CacheError::DigestMismatch);
        }
        output[..slot.length].copy_from_slice(&slot.bytes[..slot.length]);
        Ok(GetResult {
            outcome: GetOutcome::Hit,
            identity: observed,
            bytes_read: slot.length,
        })
    }

    pub fn remove(
        &mut self,
        run_epoch: u64,
        handle: CacheHandle,
    ) -> Result<RemoveOutcome, CacheError> {
        self.check_operation()?;
        self.check_handle(run_epoch, handle)?;
        let slot = self
            .slots
            .get_mut(usize::from(handle.slot))
            .ok_or(CacheError::InvalidHandle)?;
        if !slot.occupied
            || slot.generation != handle.generation
            || slot.identity != handle.identity
        {
            return Ok(RemoveOutcome::Missing);
        }
        self.retained_bytes -= slot.length;
        slot.occupied = false;
        Ok(RemoveOutcome::Removed)
    }

    fn check_operation(&self) -> Result<(), CacheError> {
        if self.cancelled {
            Err(CacheError::Cancelled)
        } else if !self.available {
            Err(CacheError::ProviderUnavailable)
        } else {
            Ok(())
        }
    }

    fn check_handle(&self, run_epoch: u64, handle: CacheHandle) -> Result<(), CacheError> {
        if handle.provider_epoch != self.provider_epoch {
            Err(CacheError::WrongProvider)
        } else if handle.run_epoch != run_epoch {
            Err(CacheError::WrongRun)
        } else {
            Ok(())
        }
    }

    fn expire(&mut self, now_tick: u64) {
        for slot in &mut self.slots {
            if slot.occupied && now_tick >= slot.expires_at_tick {
                self.retained_bytes -= slot.length;
                slot.occupied = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(run_epoch: u64, tick: u64) -> PutRequest {
        PutRequest {
            run_epoch,
            now_tick: tick,
            retention_ticks: 10,
            maximum_blob_bytes: 16,
            sensitivity: CacheSensitivity::Restricted,
        }
    }

    #[test]
    fn put_hit_remove_and_expiry_are_exact() {
        let mut cache = CacheStore::<2, 16>::new(7, 32, 20, CacheSensitivity::Restricted);
        let put = cache.put(request(3, 1), b"blob").unwrap();
        let mut output = [0; 16];
        let hit = cache
            .get(
                GetRequest {
                    run_epoch: 3,
                    now_tick: 2,
                    maximum_blob_bytes: 16,
                    handle: put.handle,
                },
                &mut output,
            )
            .unwrap();
        assert_eq!(hit.outcome, GetOutcome::Hit);
        assert_eq!(&output[..hit.bytes_read], b"blob");
        assert_eq!(cache.remove(3, put.handle).unwrap(), RemoveOutcome::Removed);
        assert_eq!(
            cache
                .get(
                    GetRequest {
                        run_epoch: 3,
                        now_tick: 3,
                        maximum_blob_bytes: 16,
                        handle: put.handle,
                    },
                    &mut output,
                )
                .unwrap()
                .outcome,
            GetOutcome::Miss
        );

        let expiring = cache.put(request(3, 4), b"short").unwrap();
        assert_eq!(
            cache
                .get(
                    GetRequest {
                        run_epoch: 3,
                        now_tick: 14,
                        maximum_blob_bytes: 16,
                        handle: expiring.handle,
                    },
                    &mut output,
                )
                .unwrap()
                .outcome,
            GetOutcome::Expired
        );
    }

    #[test]
    fn fifo_eviction_provider_loss_and_scope_fail_closed() {
        let mut cache = CacheStore::<2, 16>::new(7, 32, 20, CacheSensitivity::Restricted);
        let first = cache.put(request(3, 1), b"first").unwrap();
        cache.put(request(3, 2), b"second").unwrap();
        let third = cache.put(request(3, 3), b"third").unwrap();
        assert_eq!(third.evicted, Some(first.handle.identity));
        let mut output = [0; 16];
        assert_eq!(
            cache
                .get(
                    GetRequest {
                        run_epoch: 3,
                        now_tick: 4,
                        maximum_blob_bytes: 16,
                        handle: first.handle,
                    },
                    &mut output,
                )
                .unwrap()
                .outcome,
            GetOutcome::Evicted
        );
        assert_eq!(cache.remove(4, third.handle), Err(CacheError::WrongRun));
        let mut wrong = third.handle;
        wrong.provider_epoch = 8;
        assert_eq!(cache.remove(3, wrong), Err(CacheError::WrongProvider));
        cache.set_available(false);
        assert_eq!(
            cache.remove(3, third.handle),
            Err(CacheError::ProviderUnavailable)
        );
    }

    #[test]
    fn durability_capacity_sensitivity_digest_and_cancellation_are_explicit() {
        let mut cache = CacheStore::<1, 8>::new(1, 8, 10, CacheSensitivity::Restricted);
        let bounded_request = PutRequest {
            maximum_blob_bytes: 8,
            ..request(1, 0)
        };
        assert!(!cache.satisfies(CacheRequirement {
            persistence: CachePersistence::Durable,
            maximum_blob_bytes: 8,
            sensitivity: CacheSensitivity::Public,
        }));
        assert_eq!(
            cache.put(
                PutRequest {
                    sensitivity: CacheSensitivity::Secret,
                    ..bounded_request
                },
                b"x"
            ),
            Err(CacheError::SensitivityRefused)
        );
        assert_eq!(
            cache.put(bounded_request, b"123456789"),
            Err(CacheError::Oversized)
        );
        let handle = cache.put(bounded_request, b"ok").unwrap().handle;
        cache.slots[0].bytes[0] ^= 0xff;
        let mut output = [0; 16];
        assert_eq!(
            cache.get(
                GetRequest {
                    run_epoch: 1,
                    now_tick: 1,
                    maximum_blob_bytes: 16,
                    handle,
                },
                &mut output,
            ),
            Err(CacheError::DigestMismatch)
        );
        cache.cancel();
        assert_eq!(cache.remove(1, handle), Err(CacheError::Cancelled));
    }
}
