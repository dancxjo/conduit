//! Bounded evidence from a single observed Back instance, published only on commit.
//!
//! This proves numerical invocation custody, not Language legality or model quality.
//! No caller-supplied Play identifier is authenticated by this outlet. A new Play
//! requires a fresh factory/outlet: the exclusive writer can be claimed only once.
use super::*;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Maximum slots per placement, including held receipts.
pub const MAX_INVOCATION_SLOTS: usize = 128;
/// Maximum canonical input + output + invocation bytes retained per outlet.
pub const MAX_INVOCATION_BYTES: usize = 1_048_576;
const FREE: u8 = 0;
const RESERVED: u8 = 1;
const COMMITTED: u8 = 2;
const CLAIMED: u8 = 3;

struct Slot {
    state: AtomicU8,
    bytes: Box<[AtomicU8]>,
}
struct Inner {
    profile: Arc<PreparedCategoricalStep>,
    plan: Plan,
    placement: PlannedGear,
    flow: bool,
    writer_claimed: AtomicBool,
    slots: Box<[Slot]>,
    bytes_per_slot: usize,
}

/// Read handle. Cloning a handle does not duplicate or release held receipts.
#[derive(Clone)]
pub struct CategoricalInvocationOutlet(Arc<Inner>);
impl CategoricalInvocationOutlet {
    pub(super) fn prepare(
        plan: &Plan,
        placement: &PlannedGear,
        profile: Arc<PreparedCategoricalStep>,
        flow: bool,
        capacity: usize,
    ) -> Result<Self, String> {
        profile.verify_placement(placement, flow)?;
        let bytes_per_slot = (profile.input_bytes as usize)
            .checked_add(profile.output_bytes as usize)
            .and_then(|n| n.checked_add(8))
            .ok_or("categorical outlet byte overflow")?;
        let total = bytes_per_slot
            .checked_mul(capacity)
            .ok_or("categorical outlet capacity overflow")?;
        if capacity == 0 || capacity > MAX_INVOCATION_SLOTS || total > MAX_INVOCATION_BYTES {
            return Err("categorical outlet finite capacity".into());
        }
        let slots = (0..capacity)
            .map(|_| Slot {
                state: AtomicU8::new(FREE),
                bytes: (0..bytes_per_slot).map(|_| AtomicU8::new(0)).collect(),
            })
            .collect();
        Ok(Self(Arc::new(Inner {
            profile,
            plan: plan.clone(),
            placement: placement.clone(),
            flow,
            writer_claimed: AtomicBool::new(false),
            slots,
            bytes_per_slot,
        })))
    }
    /// Exact outlet membership only; this does not establish current request or Play custody.
    pub fn contains(&self, invocation: &CommittedCategoricalInvocation) -> bool {
        Arc::ptr_eq(&self.0, &invocation.inner)
    }
    pub fn capacity(&self) -> usize {
        self.0.slots.len()
    }
    /// Observed free-slot count from one bounded atomic scan. Concurrent claims
    /// and releases may change it immediately; this never grants or reserves
    /// capacity. Only the writer's reservation CAS can acquire a slot.
    pub fn available_slots(&self) -> usize {
        self.0
            .slots
            .iter()
            .filter(|slot| slot.state.load(Ordering::Acquire) == FREE)
            .count()
    }
    /// Payload bytes, excluding fixed atomic states, immutable binding and allocator overhead.
    pub fn retained_payload_bytes(&self) -> usize {
        self.0.bytes_per_slot * self.capacity()
    }
    /// Claims a committed slot. Concurrent readers receive disjoint leases.
    pub fn try_take(&self) -> Option<CommittedCategoricalInvocation> {
        self.0.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.state
                .compare_exchange(COMMITTED, CLAIMED, Ordering::Acquire, Ordering::Relaxed)
                .ok()
                .map(|_| CommittedCategoricalInvocation {
                    inner: self.0.clone(),
                    index,
                })
        })
    }
    pub(super) fn writer(
        &self,
        placement: &PlannedGear,
        profile: &Arc<PreparedCategoricalStep>,
        flow: bool,
    ) -> Result<InvocationWriter, String> {
        if &self.0.placement != placement
            || !Arc::ptr_eq(&self.0.profile, profile)
            || self.0.flow != flow
        {
            return Err("categorical outlet foreign selected placement".into());
        }
        self.0
            .writer_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .map_err(|_| String::from("categorical outlet already attached to a Back"))?;
        Ok(InvocationWriter(self.0.clone()))
    }
}

/// Opaque, non-cloneable committed execution evidence. Dropping releases its slot.
/// Exact canonical bytes are available without decoding or allocating in the Back.
pub struct CommittedCategoricalInvocation {
    inner: Arc<Inner>,
    index: usize,
}
impl CommittedCategoricalInvocation {
    pub fn profile(&self) -> &PreparedCategoricalStep {
        &self.inner.profile
    }
    pub fn plan(&self) -> &Plan {
        &self.inner.plan
    }
    pub fn placement(&self) -> &PlannedGear {
        &self.inner.placement
    }
    pub fn flow(&self) -> bool {
        self.inner.flow
    }
    pub fn input_len(&self) -> usize {
        self.inner.profile.input_bytes as usize
    }
    pub fn output_len(&self) -> usize {
        self.inner.profile.output_bytes as usize
    }
    fn copy(&self, offset: usize, target: &mut [u8], length: usize) -> Result<(), &'static str> {
        if target.len() != length {
            return Err("categorical evidence buffer length");
        }
        for (to, from) in target
            .iter_mut()
            .zip(&self.inner.slots[self.index].bytes[offset..offset + length])
        {
            *to = from.load(Ordering::Relaxed);
        }
        Ok(())
    }
    pub fn copy_input_into(&self, target: &mut [u8]) -> Result<(), &'static str> {
        self.copy(0, target, self.input_len())
    }
    pub fn copy_output_into(&self, target: &mut [u8]) -> Result<(), &'static str> {
        self.copy(self.input_len(), target, self.output_len())
    }
    pub fn invocation(&self) -> u64 {
        let mut bytes = [0; 8];
        // The slot was privately staged with precisely these fixed-size fields.
        for (to, from) in bytes
            .iter_mut()
            .zip(&self.inner.slots[self.index].bytes[self.input_len() + self.output_len()..])
        {
            *to = from.load(Ordering::Relaxed);
        }
        u64::from_le_bytes(bytes)
    }
}
impl Drop for CommittedCategoricalInvocation {
    fn drop(&mut self) {
        self.inner.slots[self.index]
            .state
            .store(FREE, Ordering::Release);
    }
}

pub(super) struct InvocationWriter(Arc<Inner>);
impl InvocationWriter {
    pub(super) fn reserve(&self) -> Option<Reservation> {
        self.0.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.state
                .compare_exchange(FREE, RESERVED, Ordering::Acquire, Ordering::Relaxed)
                .ok()
                .map(|_| Reservation {
                    inner: self.0.clone(),
                    index,
                    staged: false,
                    published: false,
                })
        })
    }
}
pub(super) struct Reservation {
    inner: Arc<Inner>,
    index: usize,
    staged: bool,
    published: bool,
}
impl Reservation {
    pub(super) fn stage(&mut self, input: &[u8], output: &[u8], invocation: u64) {
        debug_assert_eq!(input.len(), self.inner.profile.input_bytes as usize);
        debug_assert_eq!(output.len(), self.inner.profile.output_bytes as usize);
        for (to, from) in self.inner.slots[self.index].bytes.iter().zip(
            input
                .iter()
                .chain(output)
                .chain(invocation.to_le_bytes().iter()),
        ) {
            to.store(*from, Ordering::Relaxed);
        }
        self.staged = true;
    }
    pub(super) fn publish(mut self) {
        if self.staged {
            self.inner.slots[self.index]
                .state
                .store(COMMITTED, Ordering::Release);
            self.published = true;
        }
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        // Published reservations are COMMITTED; only unfinished ones are released.
        if self.published {
            return;
        }
        let _ = self.inner.slots[self.index].state.compare_exchange(
            RESERVED,
            FREE,
            Ordering::Release,
            Ordering::Relaxed,
        );
    }
}
