//! Opt-in post-drain evidence for the existing two-Host transport entrance.
use conduit_kernel::{KernelEvent, RemoteLifecycleIdentity, SignQuery, SignRetentionGap};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use conduit_std_host::InstalledRemoteFragment;

/// Owned reconstruction after execution. Its copying cost is separate from
/// kernel Play and must be included when measuring the observed entrance.
#[derive(Debug)]
pub struct RemoteKernelEvidence {
    pub traffic: Option<conduit_std_host::websocket::NativeWebSocketTraffic>,
    pub identity: KernelIdentityMap,
    pub events: Vec<KernelEvent>,
    pub remote_identities: Vec<(u32, RemoteLifecycleIdentity)>,
    pub retention_gap: Option<SignRetentionGap>,
}

#[derive(Debug)]
pub struct TwoHostKernelEvidence {
    pub phases: Vec<TwoHostPhaseTiming>,
    pub line: Option<TwoHostLineTiming>,
    pub source: RemoteKernelEvidence,
    pub sink: RemoteKernelEvidence,
}

pub(super) fn retain(runtime: &InstalledRemoteFragment) -> RemoteKernelEvidence {
    let signs = runtime.kernel_signs();
    let events: Vec<_> = signs.events().collect();
    let remote_identities = events
        .iter()
        .filter_map(|event| {
            signs
                .remote_identity(event.sequence)
                .map(|identity| (event.sequence, identity))
        })
        .collect();
    RemoteKernelEvidence {
        traffic: None,
        identity: runtime.kernel_identity().clone(),
        events,
        remote_identities,
        retention_gap: signs.retention_gap(),
    }
}

/// One observed API phase. Line timing includes both kernels and post-drain
/// evidence copying; it is not isolated socket or scheduler time.
#[derive(Debug)]
pub struct TwoHostPhaseTiming {
    pub phase: &'static str,
    pub elapsed_ns: u64,
}

pub(super) struct PhaseClock {
    last: std::time::Instant,
    phases: Vec<TwoHostPhaseTiming>,
}
impl PhaseClock {
    pub(super) fn new() -> Self {
        let last = std::time::Instant::now();
        Self {
            last,
            phases: Vec::with_capacity(6),
        }
    }
    pub(super) fn mark(clock: &mut Option<Self>, phase: &'static str) -> Result<(), String> {
        if let Some(clock) = clock {
            if clock.phases.len() == 6 {
                return Err("two-Host observation phase reservation exceeded".into());
            }
            let now = std::time::Instant::now();
            let elapsed_ns = u64::try_from(now.duration_since(clock.last).as_nanos())
                .map_err(|_| "two-Host observation clock range exceeded")?;
            clock.phases.push(TwoHostPhaseTiming { phase, elapsed_ns });
            clock.last = now;
        }
        Ok(())
    }
    pub(super) fn finish(self) -> Result<Vec<TwoHostPhaseTiming>, String> {
        if self.phases.len() != 6 {
            return Err("two-Host observation phase coverage incomplete".into());
        }
        Ok(self.phases)
    }
}

/// Source-owner monotonic wall intervals for one established Line. Steady time
/// excludes the first Value. Shutdown includes both owner disposal and retained
/// kernel evidence construction; caller-held output/evidence disposal is later.
#[derive(Clone, Debug, serde::Serialize)]
pub struct TwoHostLineTiming {
    pub values: u32,
    pub connection_and_session_ns: u64,
    pub first_value_ns: u64,
    pub steady_values_ns: u64,
    pub shutdown_and_retention_ns: u64,
}
pub(super) struct LineClock {
    start: std::time::Instant,
    ready: Option<std::time::Instant>,
    first: Option<std::time::Instant>,
    transfers_done: Option<std::time::Instant>,
}
impl LineClock {
    pub(super) fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
            ready: None,
            first: None,
            transfers_done: None,
        }
    }
    pub(super) fn ready(&mut self) {
        self.ready = Some(std::time::Instant::now());
    }
    pub(super) fn first(&mut self) {
        self.first = Some(std::time::Instant::now());
    }
    pub(super) fn transfers_done(&mut self) {
        self.transfers_done = Some(std::time::Instant::now());
    }
    pub(super) fn finish(self, values: u32) -> Result<TwoHostLineTiming, String> {
        fn elapsed(start: std::time::Instant, end: std::time::Instant) -> Result<u64, String> {
            let duration = end
                .checked_duration_since(start)
                .ok_or("two-Host Line clock reversed")?;
            u64::try_from(duration.as_nanos())
                .map_err(|_| "two-Host Line clock range exceeded".into())
        }
        let ready = self.ready.ok_or("two-Host Line ready timing missing")?;
        let first = self
            .first
            .ok_or("two-Host Line first Value timing missing")?;
        let done = self
            .transfers_done
            .ok_or("two-Host Line transfer timing missing")?;
        Ok(TwoHostLineTiming {
            values,
            connection_and_session_ns: elapsed(self.start, ready)?,
            first_value_ns: elapsed(ready, first)?,
            steady_values_ns: if values > 1 { elapsed(first, done)? } else { 0 },
            shutdown_and_retention_ns: elapsed(done, std::time::Instant::now())?,
        })
    }
}
