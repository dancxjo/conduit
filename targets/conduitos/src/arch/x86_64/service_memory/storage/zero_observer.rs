//! Proof-only, finite all-region witness immediately before profile Release.
//! No address/slice escapes. Caller-supplied instance is fixture correlation,
//! not authority independently authenticated by the storage Lease.
use super::*;
use core::mem::size_of;

pub(crate) const ZERO_REGIONS: usize = 12;
/// PML4, PDPT, PD, all PTs, image, exchange, output, stack, metadata,
/// optional pixels, optional scratch, supervisor trap stack (in that order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZeroRegionResult {
    pub expected_bytes: u64,
    pub checked_bytes: u64,
    pub all_zero: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ServiceZeroToken(u64);
impl ServiceZeroToken {
    pub(crate) const fn ordinal(self) -> u64 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WholeZeroWitness {
    pub token: ServiceZeroToken,
    pub profile: ServiceProfile,
    pub instance: u64,
    pub storage_bytes: u64,
    pub checked_bytes: u64,
    pub counts_valid: bool,
    pub all_zero: bool,
    pub regions: [ZeroRegionResult; ZERO_REGIONS],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZeroObserverError {
    Busy,
    InvalidIdentity,
    Exhausted,
    Missing,
    WrongToken,
    Incomplete,
}
#[derive(Clone, Copy)]
struct Arm {
    token: ServiceZeroToken,
    profile: ServiceProfile,
    instance: u64,
}
struct State {
    next: u64,
    arm: Option<Arm>,
    result: Option<WholeZeroWitness>,
}
struct Observer {
    lock: AtomicBool,
    state: UnsafeCell<State>,
}
// All access is under a finite try-lock; no IRQ can spin on an interrupted owner.
unsafe impl Sync for Observer {}
static OBSERVER: Observer = Observer {
    lock: AtomicBool::new(false),
    state: UnsafeCell::new(State {
        next: 1,
        arm: None,
        result: None,
    }),
};
pub(crate) const SERVICE_ZERO_OBSERVER_BYTES: usize = size_of::<Observer>();
const _: () = assert!(SERVICE_ZERO_OBSERVER_BYTES <= 512);
struct Guard<'a>(&'a Observer);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.lock.store(false, Ordering::Release);
    }
}
impl Observer {
    fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> Result<R, ZeroObserverError> {
        self.lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| ZeroObserverError::Busy)?;
        let _guard = Guard(self);
        // SAFETY: exclusive try-lock protects every access; closure cannot return
        // a borrowed state because its result type is independent of that borrow.
        Ok(f(unsafe { &mut *self.state.get() }))
    }
}
pub(crate) fn arm_service_zero_witness(
    profile: ServiceProfile,
    instance: u64,
) -> Result<ServiceZeroToken, ZeroObserverError> {
    OBSERVER.with(|state| {
        if instance == 0 {
            return Err(ZeroObserverError::InvalidIdentity);
        }
        if state.arm.is_some() || state.result.is_some() {
            return Err(ZeroObserverError::Busy);
        }
        let next = state
            .next
            .checked_add(1)
            .ok_or(ZeroObserverError::Exhausted)?;
        let token = ServiceZeroToken(state.next);
        state.next = next;
        state.arm = Some(Arm {
            token,
            profile,
            instance,
        });
        Ok(token)
    })?
}
pub(crate) fn take_service_zero_witness(
    token: ServiceZeroToken,
) -> Result<WholeZeroWitness, ZeroObserverError> {
    OBSERVER.with(|state| take(state, token))?
}
fn take(state: &mut State, token: ServiceZeroToken) -> Result<WholeZeroWitness, ZeroObserverError> {
    if let Some(result) = state.result {
        if result.token != token {
            return Err(ZeroObserverError::WrongToken);
        }
        state.result = None;
        return Ok(result);
    }
    if let Some(arm) = state.arm {
        if arm.token != token {
            return Err(ZeroObserverError::WrongToken);
        }
        state.arm = None;
        return Err(ZeroObserverError::Incomplete);
    }
    Err(ZeroObserverError::Missing)
}
pub(super) fn observe_release(profile: ServiceProfile) {
    // Busy cannot occur in the intended single-CPU immediate Root arm/release
    // path. If it does, no observation is fabricated: take remains Incomplete.
    let _ = OBSERVER.with(|state| {
        let Some(arm) = state.arm else {
            return;
        };
        if arm.profile != profile {
            return;
        }
        // SAFETY: Lease::drop still holds this profile's exclusive claim. No
        // mutable view is created, so the production clean marker is untouched.
        let regions = unsafe {
            match profile {
                ServiceProfile::Compositor => scan(&*COMPOSITOR.0.get()),
                ServiceProfile::Draft => scan(&*DRAFT.0.get()),
                ServiceProfile::Home => scan(&*HOME.0.get()),
                ServiceProfile::Runtime => scan(&*RUNTIME.0.get()),
                ServiceProfile::Application => scan(&*APPLICATION.0.get()),
                ServiceProfile::Network => scan(&*NETWORK.0.get()),
                ServiceProfile::OrdinaryRuntime => scan(&*ORDINARY_RUNTIME.0.get()),
                ServiceProfile::Mask => scan(&*MASK.0.get()),
                ServiceProfile::Tls => scan(&*TLS.0.get()),
            }
        };
        let storage_bytes = u64::from(reserved(profile));
        let checked = regions
            .iter()
            .try_fold(0u64, |total, r| total.checked_add(r.checked_bytes));
        let counts_valid = checked == Some(storage_bytes)
            && regions.iter().all(|r| r.checked_bytes == r.expected_bytes);
        state.arm = None;
        state.result = Some(WholeZeroWitness {
            token: arm.token,
            profile,
            instance: arm.instance,
            storage_bytes,
            checked_bytes: checked.unwrap_or(0),
            counts_valid,
            all_zero: counts_valid && regions.iter().all(|r| r.all_zero),
            regions,
        });
    });
}
fn bytes(bytes: &[u8]) -> ZeroRegionResult {
    let mut nonzero = 0u8;
    for byte in bytes {
        // Volatile observation must inspect all initialized bytes, not merely
        // prove a compiler-known fill. Never inspect uninitialized padding.
        nonzero |= unsafe { core::ptr::read_volatile(byte) };
    }
    ZeroRegionResult {
        expected_bytes: bytes.len() as u64,
        checked_bytes: bytes.len() as u64,
        all_zero: nonzero == 0,
    }
}
fn tables(tables: &[Table]) -> ZeroRegionResult {
    let mut nonzero = 0u64;
    for table in tables {
        for word in &table.0 {
            nonzero |= unsafe { core::ptr::read_volatile(word) };
        }
    }
    let count = core::mem::size_of_val(tables) as u64;
    ZeroRegionResult {
        expected_bytes: count,
        checked_bytes: count,
        all_zero: nonzero == 0,
    }
}
fn scan<
    const I: usize,
    const O: usize,
    const S: usize,
    const M: usize,
    const P: usize,
    const A: usize,
    const T: usize,
>(
    slot: &Slot<I, O, S, M, P, A, T>,
) -> [ZeroRegionResult; ZERO_REGIONS] {
    [
        tables(core::slice::from_ref(&slot.pml4)),
        tables(core::slice::from_ref(&slot.pdpt)),
        tables(core::slice::from_ref(&slot.pd)),
        tables(&slot.pt),
        bytes(&slot.image.0),
        bytes(&slot.exchange.0),
        bytes(&slot.output.0),
        bytes(&slot.stack.0),
        bytes(&slot.metadata.0),
        bytes(&slot.pixels.0),
        bytes(&slot.scratch.0),
        bytes(&slot.trap.0),
    ]
}
#[cfg(test)]
#[path = "zero_observer_tests.rs"]
mod tests;
