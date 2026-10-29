//! Allocation-free cooperative arbitration, not ambient flash authority.
//!
//! A provider adapter must own WaitGuard only across its actual timed suspension,
//! own PulseGuard across the entire capture (including cancellation retirement),
//! and retain FlashPermit through a synchronous flash operation. Durations are
//! worst-case bounds in one actual monotonic clock domain. This module cannot
//! establish those electrical/timing facts from caller-supplied timestamps.
//! No providers are wired to this gate yet; permits alone authorize no device.
use core::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Bounds,
    Clock,
    Exhausted,
    Overlap,
    Stale,
    Deadline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Window {
    generation: u64,
    deadline: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    now: u64,
    generation: u64,
    wait: Option<Window>,
    pulse: Option<Window>,
    flash: Option<u64>,
}
/// One local cooperation domain. RefCell intentionally prevents shared threaded
/// access; independent scoped adapters borrow the same Gate without allocation.
pub(crate) struct Gate {
    state: RefCell<State>,
    maximum_wait: u32,
    maximum_pulse: u32,
    maximum_flash: u32,
}
/// Affine ownership of one actual suspended wait, never Clone or deserializable.
#[must_use = "dropping the guard ends the live cooperative wait"]
pub(crate) struct WaitGuard<'gate> {
    gate: &'gate Gate,
    window: Window,
}
#[must_use = "capture stays fenced until the guard is dropped after retirement"]
pub(crate) struct PulseGuard<'gate> {
    gate: &'gate Gate,
    window: Window,
}
/// Synchronous scoped permit. Its borrow prevents the owning wait from ending
/// while flash is active. No public constructor or Clone implementation exists.
#[must_use = "retain this permit throughout the synchronous flash operation"]
pub(crate) struct FlashPermit<'wait, 'gate> {
    wait: &'wait WaitGuard<'gate>,
    generation: u64,
}
impl Gate {
    pub(crate) fn new(
        maximum_wait: u32,
        maximum_pulse: u32,
        maximum_flash: u32,
    ) -> Result<Self, Refusal> {
        if maximum_wait == 0
            || maximum_pulse == 0
            || maximum_flash == 0
            || maximum_flash > maximum_wait
        {
            return Err(Refusal::Bounds);
        }
        Ok(Self {
            state: RefCell::new(State {
                now: 0,
                generation: 0,
                wait: None,
                pulse: None,
                flash: None,
            }),
            maximum_wait,
            maximum_pulse,
            maximum_flash,
        })
    }
    fn window(
        &self,
        state: &State,
        now: u64,
        deadline: u64,
        maximum: u32,
    ) -> Result<Window, Refusal> {
        if now < state.now {
            return Err(Refusal::Clock);
        }
        let duration = deadline.checked_sub(now).ok_or(Refusal::Bounds)?;
        if duration == 0 || duration > u64::from(maximum) {
            return Err(Refusal::Bounds);
        }
        Ok(Window {
            generation: state.generation.checked_add(1).ok_or(Refusal::Exhausted)?,
            deadline,
        })
    }
    pub(crate) fn begin_wait(&self, now: u64, deadline: u64) -> Result<WaitGuard<'_>, Refusal> {
        let mut state = self.state.borrow_mut();
        let window = self.window(&state, now, deadline, self.maximum_wait)?;
        if state.wait.is_some() || state.flash.is_some() {
            return Err(Refusal::Overlap);
        }
        state.now = now;
        state.generation = window.generation;
        state.wait = Some(window);
        Ok(WaitGuard { gate: self, window })
    }
    pub(crate) fn begin_pulse(&self, now: u64, deadline: u64) -> Result<PulseGuard<'_>, Refusal> {
        let mut state = self.state.borrow_mut();
        let window = self.window(&state, now, deadline, self.maximum_pulse)?;
        if state.pulse.is_some() || state.flash.is_some() {
            return Err(Refusal::Overlap);
        }
        state.now = now;
        state.generation = window.generation;
        state.pulse = Some(window);
        Ok(PulseGuard { gate: self, window })
    }
}
impl WaitGuard<'_> {
    pub(crate) fn try_flash(
        &self,
        now: u64,
        maximum_duration: u32,
    ) -> Result<FlashPermit<'_, '_>, Refusal> {
        let mut state = self.gate.state.borrow_mut();
        if now < state.now {
            return Err(Refusal::Clock);
        }
        if state.wait != Some(self.window) {
            return Err(Refusal::Stale);
        }
        if maximum_duration == 0 || maximum_duration > self.gate.maximum_flash {
            return Err(Refusal::Bounds);
        }
        let end = now
            .checked_add(u64::from(maximum_duration))
            .ok_or(Refusal::Deadline)?;
        if end > self.window.deadline {
            return Err(Refusal::Deadline);
        }
        if state.pulse.is_some() || state.flash.is_some() {
            return Err(Refusal::Overlap);
        }
        let generation = state.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
        state.now = now;
        state.generation = generation;
        state.flash = Some(generation);
        Ok(FlashPermit {
            wait: self,
            generation,
        })
    }
}
impl Drop for WaitGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.state.borrow_mut();
        if state.wait == Some(self.window) {
            state.wait = None;
        }
    }
}
impl Drop for PulseGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.state.borrow_mut();
        if state.pulse == Some(self.window) {
            state.pulse = None;
        }
    }
}
impl Drop for FlashPermit<'_, '_> {
    fn drop(&mut self) {
        let mut state = self.wait.gate.state.borrow_mut();
        if state.flash == Some(self.generation) {
            state.flash = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gate() -> Gate {
        Gate::new(100, 20, 10).unwrap()
    }
    fn unchanged<T>(gate: &Gate, call: impl FnOnce() -> Result<T, Refusal>, error: Refusal) {
        let before = *gate.state.borrow();
        assert!(matches!(call(), Err(e) if e == error));
        assert_eq!(*gate.state.borrow(), before);
    }
    #[test]
    fn independent_wait_and_capture_guards_fence_flash_until_capture_retirement() {
        let gate = gate();
        let wait = gate.begin_wait(10, 100).unwrap();
        let pulse = gate.begin_pulse(11, 20).unwrap();
        unchanged(&gate, || wait.try_flash(12, 10), Refusal::Overlap);
        unchanged(&gate, || gate.begin_pulse(12, 21), Refusal::Overlap);
        drop(pulse);
        let flash = wait.try_flash(12, 10).unwrap();
        unchanged(&gate, || gate.begin_pulse(13, 22), Refusal::Overlap);
        unchanged(&gate, || wait.try_flash(13, 1), Refusal::Overlap);
        drop(flash);
        let pulse = gate.begin_pulse(13, 22).unwrap();
        drop(wait); // The independent capture borrow remains valid.
        assert!(gate.state.borrow().wait.is_none());
        assert!(gate.state.borrow().pulse.is_some());
        drop(pulse);
    }
    #[test]
    fn dropping_cancelled_wait_or_capture_clears_only_its_live_scope() {
        let gate = gate();
        let pulse = gate.begin_pulse(1, 5).unwrap();
        {
            let _wait = gate.begin_wait(2, 20).unwrap();
        }
        assert!(gate.state.borrow().wait.is_none());
        assert!(gate.state.borrow().pulse.is_some());
        drop(pulse);
        let wait = gate.begin_wait(3, 20).unwrap();
        {
            let _permit = wait.try_flash(4, 5).unwrap();
        }
        assert!(gate.state.borrow().flash.is_none());
        drop(wait);
        assert!(gate.state.borrow().wait.is_none());
        assert!(gate.begin_wait(5, 30).is_ok());
    }
    #[test]
    fn exact_deadline_slack_is_allowed_without_extending_the_wait() {
        let gate = gate();
        let wait = gate.begin_wait(10, 30).unwrap();
        unchanged(&gate, || wait.try_flash(21, 10), Refusal::Deadline);
        unchanged(&gate, || wait.try_flash(31, 1), Refusal::Deadline);
        let permit = wait.try_flash(20, 10).unwrap();
        drop(permit);
        unchanged(&gate, || wait.try_flash(20, 0), Refusal::Bounds);
        unchanged(&gate, || wait.try_flash(20, 11), Refusal::Bounds);
        assert_eq!(gate.state.borrow().wait.unwrap().deadline, 30);
    }
    #[test]
    fn stale_clock_overlap_and_invalid_extents_refuse_atomically() {
        assert!(Gate::new(0, 1, 1).is_err());
        assert!(Gate::new(10, 0, 1).is_err());
        assert!(Gate::new(10, 1, 11).is_err());
        let gate = gate();
        let wait = gate.begin_wait(10, 30).unwrap();
        unchanged(&gate, || gate.begin_wait(11, 30), Refusal::Overlap);
        unchanged(&gate, || gate.begin_pulse(9, 20), Refusal::Clock);
        unchanged(&gate, || wait.try_flash(9, 1), Refusal::Clock);
        unchanged(&gate, || gate.begin_pulse(10, 31), Refusal::Bounds);
        unchanged(&gate, || gate.begin_pulse(10, 10), Refusal::Bounds);
    }
    #[test]
    fn expired_pulse_guard_still_fences_flash_until_actual_retirement() {
        let gate = gate();
        let wait = gate.begin_wait(1, 90).unwrap();
        let pulse = gate.begin_pulse(2, 5).unwrap();
        unchanged(&gate, || wait.try_flash(20, 1), Refusal::Overlap);
        drop(pulse);
        assert!(wait.try_flash(20, 1).is_ok());
    }
    #[test]
    fn stale_generation_cannot_mint_permit_or_invalidate_new_scope() {
        let gate = gate();
        let wait = gate.begin_wait(1, 20).unwrap();
        let old = wait.window;
        drop(wait);
        let current = gate.begin_wait(2, 30).unwrap();
        // Synthetic stale guard is test-only: no safe public constructor exists.
        let stale = WaitGuard {
            gate: &gate,
            window: old,
        };
        unchanged(&gate, || stale.try_flash(3, 1), Refusal::Stale);
        drop(stale);
        assert_eq!(gate.state.borrow().wait, Some(current.window));
    }
    #[test]
    fn generation_and_clock_arithmetic_never_wrap() {
        let gate = gate();
        gate.state.borrow_mut().generation = u64::MAX;
        unchanged(&gate, || gate.begin_wait(1, 2), Refusal::Exhausted);
        unchanged(&gate, || gate.begin_pulse(1, 2), Refusal::Exhausted);
        gate.state.borrow_mut().generation = u64::MAX - 1;
        let wait = gate.begin_wait(u64::MAX - 2, u64::MAX).unwrap();
        unchanged(
            &gate,
            || wait.try_flash(u64::MAX - 2, 1),
            Refusal::Exhausted,
        );
        unchanged(&gate, || wait.try_flash(u64::MAX, 1), Refusal::Deadline);
    }
}
