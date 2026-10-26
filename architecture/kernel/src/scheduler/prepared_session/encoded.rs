//! Direct canonical encoding preserves the existing owned-session admission.
use super::*;
use crate::scheduler::{HostEncodingRefusal, RemoteIngressOutcome};
impl<
        D,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
    > PreparedKernelSession<D, crate::HostedValueStore, E, N, C, P, Q, R, T, H, A>
where
    D: ReusableStepBack<P>,
    E: SignSink + SignQuery,
{
    /// Same finite one-Value-per-invocation contract, using admitted store
    /// backing as the encoder destination. No scratch copy of Face is required.
    pub fn admit_owned_input_encoded<X>(
        &mut self,
        endpoint: crate::RemoteEndpointId,
        cord: crate::CordId,
        sequence: u64,
        maximum_bytes: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<usize, X>,
    ) -> Result<RemoteIngressOutcome, HostEncodingRefusal<X>> {
        if let Some(outcome) =
            self.preflight_owned_input(endpoint, cord, sequence, maximum_bytes)?
        {
            return Ok(outcome);
        }
        let result =
            self.kernel
                .admit_remote_input_encoded(endpoint, cord, sequence, maximum_bytes, encode);
        if result.is_err() {
            self.phase = Phase::Faulted;
        } else if matches!(result, Ok(RemoteIngressOutcome::Accepted { .. })) {
            self.phase = Phase::Active;
        }
        result
    }
    /// Encoding failure has the same lifecycle effect as failed Host value
    /// storage. Its exact refusal remains visible and no completion is invented.
    pub fn store_host_value_encoded<X>(
        &mut self,
        maximum_bytes: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<usize, X>,
    ) -> Result<ValueRef, HostEncodingRefusal<X>> {
        let result = self.kernel.store_host_value_encoded(maximum_bytes, encode);
        if result.is_err() && self.phase == Phase::Active {
            self.phase = Phase::Faulted;
        }
        result
    }
}
