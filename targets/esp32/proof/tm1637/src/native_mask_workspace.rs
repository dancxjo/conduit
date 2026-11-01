//! Scheduling custody only: no Body, lease, provider or capacity authority.
//! Running workers offer a boundary after accepted child retirement. A stopped
//! task may instead lend scheduling custody while retaining failed electrical owners.
use core::sync::atomic::{AtomicU32, Ordering};
const REQUESTED: u32 = 1;
const READY: u32 = 2;
const HELD: u32 = 3;
const PHASE: u32 = 3;
const GENERATION: u32 = !PHASE;
/// One permanently associated worker. Generations never wrap or reset.
pub(crate) struct Gate(AtomicU32);
impl Gate {
    pub(crate) const fn new() -> Self {
        Self(AtomicU32::new(0))
    }
    pub(crate) fn request(&self) -> Result<Request<'_>, &'static str> {
        let old = self.0.load(Ordering::Acquire);
        if old & PHASE != 0 {
            return Err("mask-workspace-busy");
        }
        let generation = old.checked_add(4).ok_or("mask-workspace-exhausted")?;
        self.0
            .compare_exchange(
                old,
                generation | REQUESTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|_| "mask-workspace-busy")?;
        Ok(Request {
            gate: self,
            generation,
        })
    }
    /// Caller must have established an exclusive scheduling boundary. The running
    /// worker uses actual renderer/lease retirement; terminal custody is separate.
    /// A current-scope check is mandatory here and again after the pause.
    pub(crate) fn retired_boundary(&self, current: bool) -> Option<Pause<'_>> {
        let old = self.0.load(Ordering::Acquire);
        if !current {
            self.cancel();
            return None;
        }
        if old & PHASE != REQUESTED {
            return None;
        }
        self.0
            .compare_exchange(
                old,
                (old & GENERATION) | READY,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .ok()?;
        Some(Pause {
            gate: self,
            generation: old & GENERATION,
        })
    }
    /// Accepted predecessor Replaced evidence, revoked old capability and
    /// discarded old child metadata establish scheduling quiescence while the
    /// successor's acknowledged Available Show retains the physical owner.
    /// Caller must also prove drained kernel and no in-flight electrical call.
    pub(crate) fn replaced_boundary(&self, current: bool) -> Option<Pause<'_>> {
        self.retired_boundary(current)
    }
    /// A stopped task may lend scheduling custody while retaining every failed
    /// electrical owner. This attests no renderer retirement or free memory.
    pub(crate) fn terminal_boundary(&self, retained: bool) -> Result<Request<'_>, &'static str> {
        if !retained {
            return Err("mask-terminal-custody");
        }
        let request = self.request()?;
        self.retired_boundary(true)
            .ok_or("mask-terminal-boundary")?;
        Ok(request)
    }
    pub(crate) fn idle(&self) -> bool {
        self.0.load(Ordering::Acquire) & PHASE == 0
    }
    /// Worker exit/revocation invalidates any pending request or held permit.
    pub(crate) fn cancel(&self) {
        self.0.fetch_and(GENERATION, Ordering::AcqRel);
    }
    fn release(&self, generation: u32) {
        let mut old = self.0.load(Ordering::Acquire);
        while old & GENERATION == generation && old & PHASE != 0 {
            match self
                .0
                .compare_exchange_weak(old, generation, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return,
                Err(now) => old = now,
            }
        }
    }
}
/// Affine pending request. Timeout/cancellation withdraws only its generation.
pub(crate) struct Request<'a> {
    gate: &'a Gate,
    generation: u32,
}
impl<'a> Request<'a> {
    pub(crate) fn ready(&self) -> bool {
        self.gate.0.load(Ordering::Acquire) == self.generation | READY
    }
    pub(crate) fn claim(self) -> Result<Permit<'a>, Self> {
        if self
            .gate
            .0
            .compare_exchange(
                self.generation | READY,
                self.generation | HELD,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return Err(self);
        }
        let permit = Permit {
            gate: self.gate,
            generation: self.generation,
        };
        core::mem::forget(self);
        Ok(permit)
    }
}
impl Drop for Request<'_> {
    fn drop(&mut self) {
        self.gate.release(self.generation);
    }
}
/// Not Clone. Holding this prevents this worker constructing its next child.
/// Drop always releases; it does not admit network work or attest free memory.
pub(crate) struct Permit<'a> {
    gate: &'a Gate,
    generation: u32,
}
impl Permit<'_> {
    pub(crate) fn held(&self) -> bool {
        self.gate.0.load(Ordering::Acquire) == self.generation | HELD
    }
}
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.gate.release(self.generation);
    }
}
pub(crate) struct Pause<'a> {
    gate: &'a Gate,
    generation: u32,
}
impl Pause<'_> {
    pub(crate) fn waiting(&self) -> bool {
        let now = self.gate.0.load(Ordering::Acquire);
        now & GENERATION == self.generation && matches!(now & PHASE, READY | HELD)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claim_requires_retired_current_boundary_and_affine_release() {
        let gate = Gate::new();
        let request = gate.request().unwrap();
        assert!(!request.ready());
        let request = request.claim().err().unwrap();
        assert!(gate.request().is_err());
        let pause = gate.retired_boundary(true).unwrap();
        assert!(request.ready());
        let permit = request.claim().ok().unwrap();
        assert!(permit.held());
        assert!(pause.waiting());
        assert!(gate.request().is_err());
        drop(permit);
        assert!(!pause.waiting());
        assert!(gate.request().is_ok());
    }
    #[test]
    fn request_drop_before_or_after_offer_withdraws_pause() {
        let gate = Gate::new();
        drop(gate.request().unwrap());
        assert!(gate.retired_boundary(true).is_none());
        let request = gate.request().unwrap();
        let pause = gate.retired_boundary(true).unwrap();
        drop(request);
        assert!(!pause.waiting());
    }
    #[test]
    fn revoked_parent_cancels_pending_and_held_generation() {
        let gate = Gate::new();
        let request = gate.request().unwrap();
        assert!(gate.retired_boundary(false).is_none());
        assert!(request.claim().is_err());
        let request = gate.request().unwrap();
        let pause = gate.retired_boundary(true).unwrap();
        let permit = request.claim().ok().unwrap();
        gate.cancel();
        assert!(!permit.held());
        assert!(!pause.waiting());
    }
    #[test]
    fn stale_request_and_permit_cannot_release_new_generation() {
        let gate = Gate::new();
        let stale_request = gate.request().unwrap();
        gate.cancel();
        let request = gate.request().unwrap();
        drop(stale_request);
        let pause = gate.retired_boundary(true).unwrap();
        let stale_permit = request.claim().ok().unwrap();
        gate.cancel();
        let request = gate.request().unwrap();
        let next_pause = gate.retired_boundary(true).unwrap();
        drop(stale_permit);
        assert!(!pause.waiting());
        assert!(request.ready());
        assert!(next_pause.waiting());
        drop(request);
        assert!(!next_pause.waiting());
    }
    #[test]
    fn replacement_boundary_preserves_affine_generation_and_revocation() {
        let gate = Gate::new();
        let request = gate.request().unwrap();
        let pause = gate.replaced_boundary(true).unwrap();
        let permit = request.claim().ok().unwrap();
        assert!(pause.waiting() && permit.held());
        gate.cancel();
        assert!(!pause.waiting() && !permit.held());
        let next = gate.request().unwrap();
        assert!(gate.replaced_boundary(false).is_none());
        assert!(!next.ready());
    }
    #[test]
    fn terminal_custody_is_not_inferred_from_failure_and_excludes_overlap() {
        let gate = Gate::new();
        assert!(gate.terminal_boundary(false).is_err());
        assert!(gate.idle());
        let request = gate.terminal_boundary(true).unwrap();
        assert!(request.ready());
        assert!(!gate.idle());
        assert!(gate.terminal_boundary(true).is_err());
        let permit = request.claim().ok().unwrap();
        assert!(permit.held());
        assert!(!gate.idle());
        gate.cancel();
        let next = gate.terminal_boundary(true).unwrap();
        drop(permit);
        assert!(next.ready());
        drop(next);
        assert!(gate.idle());
    }
    #[test]
    fn exhausted_generation_never_wraps() {
        let gate = Gate(AtomicU32::new(GENERATION));
        assert!(gate.request().is_err());
    }
}
