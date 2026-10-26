//! Remote ingress shares the same admission and ownership path whether its
//! canonical bytes are copied or encoded directly into the admitted value slot.
use super::*;
#[cfg(feature = "alloc")]
#[derive(Debug, PartialEq, Eq)]
pub enum HostEncodingRefusal<E> {
    Scheduler(SchedulerError),
    Encoding(crate::EncodedValueRefusal<E>),
}
#[cfg(feature = "alloc")]
impl<E> From<SchedulerError> for HostEncodingRefusal<E> {
    fn from(error: SchedulerError) -> Self {
        Self::Scheduler(error)
    }
}
impl<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
    > FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>
where
    D: StepBack<P>,
    S: ValueStorage,
    E: SignSink,
{
    /// Full performs no allocation or sequence advance. Byte admission and
    /// encoded admission below use this one shared transactional path.
    pub fn admit_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        self.admit_remote_input_with(endpoint, cord, sequence, bytes.len(), |values| {
            values.store(bytes).map_err(Into::into)
        })
    }
    fn admit_remote_input_with<X: From<SchedulerError>>(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        maximum_bytes: usize,
        store: impl FnOnce(&mut S) -> Result<ValueRef, X>,
    ) -> Result<RemoteIngressOutcome, X> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled.into());
        }
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess.into());
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (sink_node, sink_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Remote(candidate), CordEndpoint::Local { node, port })
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess.into()),
        };
        let state = self
            .cords
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if state.producer_closed || state.next_remote_sequence != sequence {
            return Err(SchedulerError::RemoteSequenceRejected.into());
        }
        let byte_len =
            u32::try_from(maximum_bytes).map_err(|_| SchedulerError::QueueByteCapacityExceeded)?;
        if !self.payload_admissible(spec, state, byte_len)? {
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        self.ensure_sign_capacity(1)?;
        self.ensure_remote_sign_capacity(1)?;
        let value = store(&mut self.values)?;
        match self.push(cord_index, value) {
            Ok(Some(superseded)) => self
                .values
                .release(superseded)
                .map_err(SchedulerError::from)?,
            Ok(None) => {}
            Err(error) => {
                self.values.release(value).map_err(SchedulerError::from)?;
                return Err(error.into());
            }
        }
        self.cords[cord_index].next_remote_sequence = next_sequence;
        self.set_ready(usize::from(sink_node.0), true);
        self.signs
            .record_remote(
                sink_node,
                sink_port,
                KernelEventKind::RemoteInputAdmitted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord,
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence,
                },
            )
            .map_err(SchedulerError::from)?;
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }
}
#[cfg(feature = "alloc")]
impl<
        D,
        V,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
    > FixedScheduler<D, V, E, N, C, P, Q, R, T, H, A>
where
    D: StepBack<P>,
    V: crate::CanonicalEncodedValueStorage,
    E: SignSink,
{
    /// Reserve the full finite ceiling before invoking the encoder. Refusal or
    /// pressure never calls it; failed encoding publishes no value or sequence.
    pub fn admit_remote_input_encoded<X>(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        maximum_bytes: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<usize, X>,
    ) -> Result<RemoteIngressOutcome, HostEncodingRefusal<X>> {
        self.admit_remote_input_with(endpoint, cord, sequence, maximum_bytes, |values| {
            values
                .store_encoded(maximum_bytes, encode)
                .map_err(HostEncodingRefusal::Encoding)
        })
    }
    /// The returned value remains Host-owned until the existing completion or
    /// discard protocol consumes it. No separate encoded buffer is allocated.
    pub fn store_host_value_encoded<X>(
        &mut self,
        maximum_bytes: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<usize, X>,
    ) -> Result<ValueRef, HostEncodingRefusal<X>> {
        if self.cancelled {
            return Err(HostEncodingRefusal::Scheduler(SchedulerError::Cancelled));
        }
        self.values
            .store_encoded(maximum_bytes, encode)
            .map_err(HostEncodingRefusal::Encoding)
    }
}
