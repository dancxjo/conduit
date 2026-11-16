//! Only the selected archive-backed sink exposes this lossless owner transfer.
use super::*;
use crate::live_sign_archive::{LiveSignLog, PreparedPrefixTransfer, TransferRefusal};

impl<
        D: StepBack<P>,
        V: ValueStorage,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const K: usize,
        const M: usize,
        const F: usize,
        const U: usize,
        const Z: usize,
        const E: usize,
        const RUNS: usize,
        const STREAMS: usize,
    > FixedScheduler<D, V, LiveSignLog<E, RUNS, STREAMS>, N, C, P, Q, R, T, H, K, M, F, U, Z>
{
    /// Exclusively borrow this kernel while the preflighted exact Sign prefix
    /// moves into its separately admitted archive. No Back, value, queue,
    /// request counter, readiness, cancellation or execution origin changes.
    pub fn prepare_sign_prefix_transfer(
        &mut self,
    ) -> Result<PreparedPrefixTransfer<'_, E, RUNS, STREAMS>, TransferRefusal> {
        self.signs.prepare_prefix_transfer()
    }
}
