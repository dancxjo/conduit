//! Cooperative custody of one independently checked IMAGE heap arena.
//!
//! This is neither allocator containment nor a measurement of preparation peak.
//! Root may charge existing allocation once, then hold the entire remainder as
//! exclusive preparation/execution workspace. It grants no execution authority.
use core::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Empty,
    Capacity,
}

/// One boot-local memory owner. Borrowed leases cannot outlive this counter.
/// No allocation, reference counting, or concurrent counter access is needed.
pub struct StagedImageMemory {
    used: Cell<usize>,
    limit: usize,
}

impl StagedImageMemory {
    /// Establish custody of the original checked IMAGE's physical heap arena.
    ///
    /// # Safety
    /// Root must obtain `heap_arena_bytes` from the independently verified
    /// configuration/IMAGE which actually constructed its allocator. It must
    /// create exactly one owner for that arena and retain it for all users.
    /// All preexisting allocation must be charged once before reserving the
    /// remainder; no other owner may reuse that backing. Leases are cooperative
    /// backing custody, not proof that a factory fits or authority to enlarge
    /// the allocator, a resource grant, or a preparation delegation.
    pub unsafe fn verified_heap(heap_arena_bytes: usize) -> Result<Self, Refusal> {
        if heap_arena_bytes == 0 {
            return Err(Refusal::Empty);
        }
        Ok(Self {
            used: Cell::new(0),
            limit: heap_arena_bytes,
        })
    }

    pub fn remaining_bytes(&self) -> usize {
        self.limit - self.used.get()
    }

    /// Reserve a complete nonempty backing envelope before its use. Existing
    /// allocator backing may be charged here once without allocating it again.
    pub fn reserve(&self, bytes: usize) -> Result<ImageMemoryLease<'_>, Refusal> {
        if bytes == 0 {
            return Err(Refusal::Empty);
        }
        let next = self
            .used
            .get()
            .checked_add(bytes)
            .ok_or(Refusal::Capacity)?;
        if next > self.limit {
            return Err(Refusal::Capacity);
        }
        self.used.set(next);
        Ok(ImageMemoryLease { owner: self, bytes })
    }

    /// Retain exclusive custody of all currently uncharged arena backing.
    /// Keep this lease throughout preparation and execution; no peak estimate
    /// or shrink operation is implied. Competing reservations then refuse.
    pub fn reserve_remaining(&self) -> Result<ImageMemoryLease<'_>, Refusal> {
        self.reserve(self.remaining_bytes())
    }
}

/// Affine, non-Clone backing custody. Drop returns exactly its original charge.
#[must_use = "retain the lease for the entire backing lifetime"]
pub struct ImageMemoryLease<'a> {
    owner: &'a StagedImageMemory,
    bytes: usize,
}

impl ImageMemoryLease<'_> {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Drop for ImageMemoryLease<'_> {
    fn drop(&mut self) {
        // Only reserve constructs leases; none can be copied or shrunk.
        self.owner.used.set(self.owner.used.get() - self.bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_and_exclusive_workspace_release_without_overflow_or_reuse() {
        // SAFETY: this test is the sole owner of its modeled arena.
        let arena = unsafe { StagedImageMemory::verified_heap(128) }.unwrap();
        assert!(matches!(arena.reserve(0), Err(Refusal::Empty)));
        let baseline = arena.reserve(24).unwrap();
        assert!(matches!(arena.reserve(usize::MAX), Err(Refusal::Capacity)));
        assert_eq!(arena.remaining_bytes(), 104);
        {
            let workspace = arena.reserve_remaining().unwrap();
            assert_eq!(workspace.bytes(), 104);
            assert!(matches!(arena.reserve(1), Err(Refusal::Capacity)));
            assert!(matches!(arena.reserve_remaining(), Err(Refusal::Empty)));
        }
        assert_eq!(arena.remaining_bytes(), 104);
        drop(baseline);
        assert_eq!(arena.remaining_bytes(), 128);
        assert_eq!(arena.reserve_remaining().unwrap().bytes(), 128);
        // SAFETY: no arena is established for the rejected empty description.
        assert!(matches!(
            unsafe { StagedImageMemory::verified_heap(0) },
            Err(Refusal::Empty)
        ));
    }
}
