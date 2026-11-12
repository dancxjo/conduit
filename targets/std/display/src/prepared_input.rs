//! Readiness of one actual, settled SDL window input stream. This token is not
//! input authority, a Host offer, a Show, or an IMAGE memory reservation.
use crate::InitializedDisplay;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone)]
pub struct PreparedNativeInput {
    lease: Arc<InputLease>,
}
struct InputLease {
    display: InitializedDisplay,
    live: AtomicBool,
    claimed: AtomicBool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInputClaimRefusal {
    Lost,
    AlreadyClaimed,
}
/// One Host's exclusive software input claim. The driver still owns its SDL
/// event pump. Dropping the claim retires these tokens, requiring fresh input
/// preparation; it never revokes the physical display or changes Body state.
pub struct NativeInputHostClaim {
    input: PreparedNativeInput,
}
impl PreparedNativeInput {
    // Only Scanout, after actual empty-queue observation and held-button
    // reconciliation, may construct readiness. Never decode this from metadata.
    pub(crate) fn prepared(display: InitializedDisplay) -> Self {
        Self {
            lease: Arc::new(InputLease {
                display,
                live: AtomicBool::new(true),
                claimed: AtomicBool::new(false),
            }),
        }
    }
    pub fn is_live(&self) -> bool {
        self.lease.live.load(Ordering::Acquire) && self.lease.display.is_live()
    }
    pub fn matches_display(&self, display: &InitializedDisplay) -> bool {
        self.is_live() && self.lease.display.same_instance(display)
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.lease, &other.lease)
    }
    /// Requested backing of this shared readiness allocation, counted ONCE.
    /// The original display lease, SDL/controller queues, event pump, producer
    /// validation and stack have separate caller custody. Cloning allocates none.
    pub const fn owned_backing_bytes() -> usize {
        std::mem::size_of::<InputLease>() + 2 * std::mem::size_of::<usize>()
    }
    pub fn claim_host(&self) -> Result<NativeInputHostClaim, NativeInputClaimRefusal> {
        if !self.is_live() {
            return Err(NativeInputClaimRefusal::Lost);
        }
        self.lease
            .claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| NativeInputClaimRefusal::AlreadyClaimed)?;
        if !self.is_live() {
            self.lease.claimed.store(false, Ordering::Release);
            return Err(NativeInputClaimRefusal::Lost);
        }
        Ok(NativeInputHostClaim {
            input: self.clone(),
        })
    }
    pub(crate) fn revoke(&self) {
        self.lease.live.store(false, Ordering::Release);
    }
}
impl NativeInputHostClaim {
    pub fn prepared(&self) -> &PreparedNativeInput {
        &self.input
    }
    pub fn is_live(&self) -> bool {
        self.input.is_live()
    }
}
impl Drop for NativeInputHostClaim {
    fn drop(&mut self) {
        self.input.revoke();
        self.input.lease.claimed.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests;
