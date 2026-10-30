//! Safe worker-side operations used by the eventual C ABI bindings. No raw
//! pointer, borrowed buffer or socket escapes a callback's synchronous extent.
use crate::BridgeAccess;
use conduit_zenoh_pico_platform::{
    Endpoint, Fault, Phase, Progress, Refusal, SocketBridge, SocketGeneration,
};

/// Actual boot-local monotonic clock and bounded worker sleep/yield. This must
/// run on the admitted blocking worker, never the sampler's executor. A failed
/// timer/sleep reports a fault; it must not invent clock progress.
pub trait WorkerRuntime {
    fn now_ms(&self) -> u64;
    fn yield_worker(&self) -> Result<(), Fault>;
}

/// One blocking worker and one socket slot. The bridge and real network owner
/// remain external. This value creates neither a thread nor resource admission.
pub struct SocketWorker<'a, A, R, const RX: usize, const TX: usize> {
    bridge: &'a A,
    runtime: R,
    blocking: bool,
    close_timeout_ms: u32,
}
impl<'a, A: BridgeAccess<RX, TX>, R: WorkerRuntime, const RX: usize, const TX: usize>
    SocketWorker<'a, A, R, RX, TX>
{
    pub fn new(bridge: &'a A, runtime: R, close_timeout_ms: u32) -> Result<Self, Refusal> {
        if close_timeout_ms == 0 {
            return Err(Refusal::Extent);
        }
        Ok(Self {
            bridge,
            runtime,
            blocking: true,
            close_timeout_ms,
        })
    }
    pub(crate) fn generation(&self, generation: u32) -> Result<SocketGeneration, Refusal> {
        self.bridge.with(|b| {
            b.generation()
                .filter(|g| g.get() == generation)
                .ok_or(Refusal::Stale)
        })
    }
    fn deadline(&self, timeout_ms: u32) -> Result<u64, Refusal> {
        self.runtime
            .now_ms()
            .checked_add(u64::from(timeout_ms))
            .ok_or(Refusal::Exhausted)
    }
    fn wait<T>(
        &self,
        deadline: u64,
        mut operation: impl FnMut(&mut SocketBridge<RX, TX>) -> Result<Option<T>, Refusal>,
    ) -> Result<T, Refusal> {
        // Always allow one immediate poll, including nonblocking operations.
        if let Some(value) = self.bridge.with(&mut operation)? {
            return Ok(value);
        }
        loop {
            if self.runtime.now_ms() >= deadline {
                return Err(Refusal::Fault(Fault::TimedOut));
            }
            self.runtime.yield_worker().map_err(Refusal::Fault)?;
            if self.runtime.now_ms() >= deadline {
                return Err(Refusal::Fault(Fault::TimedOut));
            }
            if let Some(value) = self.bridge.with(&mut operation)? {
                return Ok(value);
            }
        }
    }
    pub fn connect(&mut self, endpoint: Endpoint, timeout_ms: u32) -> Result<u32, Refusal> {
        if timeout_ms == 0 {
            return Err(Refusal::Fault(Fault::TimedOut));
        }
        let deadline = self.deadline(timeout_ms)?;
        let generation = self
            .bridge
            .with(|b| b.request_connect(endpoint, deadline))?;
        self.blocking = true;
        let result = self.wait(deadline, |b| {
            if b.generation() != Some(generation) {
                return Err(Refusal::Stale);
            }
            match b.phase() {
                Phase::Open => Ok(Some(generation.get())),
                Phase::Connecting => Ok(None),
                Phase::Failed(fault) => Err(Refusal::Fault(fault)),
                _ => Err(Refusal::State),
            }
        });
        if result.is_err() {
            // No C handle was published, but the real attempt still needs
            // retirement. A failed bounded close leaves the slot quarantined.
            let _ = self.close(generation.get());
        }
        result
    }
    /// Request retirement and wait a finite interval for actual owner abort.
    /// Timeout/failure leaves Closing intact; it never reports a reusable slot.
    pub fn close(&mut self, generation: u32) -> Result<(), Refusal> {
        let generation = self.generation(generation)?;
        // Even an exhausted clock must request retirement. Saturation makes
        // the wait expire immediately at the clock ceiling, leaving Closing.
        let deadline = self
            .runtime
            .now_ms()
            .saturating_add(u64::from(self.close_timeout_ms));
        self.bridge.with(|b| b.request_close(generation))?;
        self.wait(deadline, |b| match b.generation() {
            None => Ok(Some(())),
            Some(current) if current == generation => Ok(None),
            _ => Err(Refusal::Stale),
        })
    }
    pub fn set_blocking(&mut self, generation: u32, blocking: bool) -> Result<(), Refusal> {
        self.generation(generation)?;
        self.blocking = blocking;
        Ok(())
    }
    fn transfer(
        &self,
        timeout_ms: u32,
        operation: impl FnMut(&mut SocketBridge<RX, TX>) -> Result<Progress, Refusal>,
    ) -> Result<usize, Refusal> {
        let deadline = self.deadline(if self.blocking { timeout_ms } else { 0 })?;
        self.transfer_until(deadline, operation)
    }
    fn transfer_until(
        &self,
        deadline: u64,
        mut operation: impl FnMut(&mut SocketBridge<RX, TX>) -> Result<Progress, Refusal>,
    ) -> Result<usize, Refusal> {
        self.wait(deadline, |b| match operation(b)? {
            Progress::Bytes(n) => Ok(Some(n)),
            Progress::Eof => Ok(Some(0)),
            Progress::Pending => Ok(None),
        })
    }
    /// Returns any accepted prefix immediately. A later fault cannot turn that
    /// prefix into a timeout/retry of the original intact buffer.
    pub fn read(
        &mut self,
        generation: u32,
        bytes: &mut [u8],
        timeout_ms: u32,
    ) -> Result<usize, Refusal> {
        let generation = self.generation(generation)?;
        self.transfer(timeout_ms, |b| b.read(generation, bytes))
    }
    /// Success means fixed local TX-buffer custody, never remote delivery or a
    /// durable recording ACK. No caller buffer is retained after this returns.
    pub fn write(
        &mut self,
        generation: u32,
        bytes: &[u8],
        timeout_ms: u32,
    ) -> Result<usize, Refusal> {
        let generation = self.generation(generation)?;
        self.transfer(timeout_ms, |b| b.write(generation, bytes))
    }
    /// C's streamed write uses its explicit finite budget even when reads poll
    /// nonblocking. Only a known accepted prefix is returned on each call.
    pub(crate) fn write_bounded(
        &mut self,
        generation: u32,
        bytes: &[u8],
        timeout_ms: u32,
    ) -> Result<usize, Refusal> {
        let generation = self.generation(generation)?;
        self.transfer_until(self.deadline(timeout_ms)?, |b| b.write(generation, bytes))
    }
    /// Failure/EOF is readable, so accepted RX can drain before its terminal
    /// status. Deadline is ordinary lack of readiness, not a zero-byte read.
    pub fn ready(&mut self, generation: u32, timeout_ms: u32) -> Result<bool, Refusal> {
        let generation = self.generation(generation)?;
        let deadline = self.deadline(if self.blocking { timeout_ms } else { 0 })?;
        match self.wait(deadline, |b| {
            b.readable(generation).map(|ready| ready.then_some(true))
        }) {
            Err(Refusal::Fault(Fault::TimedOut)) => Ok(false),
            result => result,
        }
    }
}
