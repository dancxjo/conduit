//! Preallocated event identities and output backing for IMAGE-bound execution.
use super::*;
use conduit_core::{MonotonicClockIdentity, MonotonicInstant};
pub(super) struct PreparedClock {
    basis: MonotonicClockIdentity,
    identities: Vec<MonotonicClockIdentity>,
    samples: Vec<(u32, u64)>,
    output: Vec<ObservedKernelEvent>,
    last: u64,
    supported: bool,
    pub(super) seen: usize,
}
impl PreparedClock {
    pub(super) fn prepare<T: TimerAdapter>(
        timer: &mut T,
        host: &HostId,
        boot: &BootId,
        capacity: usize,
    ) -> Result<Self, String> {
        let initial = timer
            .monotonic_observation(host, boot)
            .ok_or("selected timer lacks prepared clock identity")?;
        if initial.clock().host_id() != host || initial.clock().boot_id() != boot {
            return Err("selected clock identity differs from Host/Boot".into());
        }
        let now = timer
            .monotonic_ticks_on(initial.clock())
            .ok_or("selected timer lacks allocation-free exact-basis sampling")?;
        if now < initial.ticks() {
            return Err("selected clock regressed during preparation".into());
        }
        Ok(Self {
            basis: initial.clock().clone(),
            identities: (0..capacity).map(|_| initial.clock().clone()).collect(),
            samples: Vec::with_capacity(capacity),
            output: Vec::with_capacity(capacity),
            last: now,
            supported: true,
            seen: 0,
        })
    }
    pub(super) fn retained_backing_bytes(&self) -> Option<usize> {
        let mut total = self.basis.retained_backing_bytes()?;
        for bytes in [
            self.identities
                .capacity()
                .checked_mul(core::mem::size_of::<MonotonicClockIdentity>())?,
            self.samples
                .capacity()
                .checked_mul(core::mem::size_of::<(u32, u64)>())?,
            self.output
                .capacity()
                .checked_mul(core::mem::size_of::<ObservedKernelEvent>())?,
        ] {
            total = total.checked_add(bytes)?;
        }
        for identity in &self.identities {
            total = total.checked_add(identity.retained_backing_bytes()?)?;
        }
        Some(total)
    }
    pub(super) fn capture<T: TimerAdapter>(
        &mut self,
        events: impl Iterator<Item = KernelEvent>,
        timer: &mut T,
    ) {
        for event in events {
            self.seen += 1;
            if !self.supported {
                continue;
            }
            let Some(ticks) = timer.monotonic_ticks_on(&self.basis) else {
                self.supported = false;
                continue;
            };
            if ticks < self.last {
                self.supported = false;
                continue;
            }
            self.last = ticks;
            if self.samples.len() < self.samples.capacity() {
                self.samples.push((event.sequence, ticks));
            }
        }
    }
    pub(super) fn finish(mut self) -> Vec<ObservedKernelEvent> {
        for ((sequence, ticks), identity) in self.samples.into_iter().zip(self.identities) {
            let local =
                MonotonicInstant::new(ticks, identity).expect("prepared identity remains valid");
            let time = BodyEventTimeObservation::captured_after(local)
                .expect("prepared scalar observation remains valid");
            self.output.push(ObservedKernelEvent { sequence, time });
        }
        self.output
    }
}
