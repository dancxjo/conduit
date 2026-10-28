//! Exact linked-Type publication with two transactional canonical value nodes.
//! Full bytes exist only during a synchronous scoped projection callback.
use super::{PublicationChannel, PublicationContract, PublicationSequence, PublicationSnapshot};
use crate::canonical_input::PreparedCanonicalInput;
use alloc::{boxed::Box, vec, vec::Vec};
use conduit_core::StructuredInfoRefusal;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct SourcePublicationBack {
    channel: PublicationChannel,
    validator: PreparedCanonicalInput,
    current: Vec<u8>,
    pending: Vec<u8>,
    maximum_events: u32,
    sequence: u64,
    events: u32,
    order: PublicationSequence,
    length: usize,
    staged: bool,
    cancelled: bool,
    compact: Option<Box<super::source_reading::CompactReading>>,
}
impl SourcePublicationBack {
    pub fn prepare(
        contract: &PublicationContract,
        maximum_events: u32,
        order: PublicationSequence,
    ) -> Result<Self, StructuredInfoRefusal> {
        // Every valid canonical value fits this Type-derived bound. Retain the
        // channel ceiling too; tightening storage does not narrow its domain.
        let bytes = crate::canonical_storage::bytes(
            contract.value_type(),
            contract.channel().maximum_bytes(),
        )?;
        let schema = contract.channel().linked_type_bytes();
        if contract.value_type().canonical_bytes()? != schema {
            return Err(StructuredInfoRefusal::WrongType);
        }
        if bytes > material::RESULT_MAXIMUM || bytes < schema.len() {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let node_bytes = bytes - schema.len();
        Ok(Self {
            channel: contract.channel(),
            validator: PreparedCanonicalInput::with_static_schema(
                contract.value_type(),
                contract.channel().linked_type_bytes(),
                bytes,
            )?,
            current: Vec::with_capacity(node_bytes),
            pending: vec![0; node_bytes],
            maximum_events,
            sequence: 0,
            events: 0,
            order,
            length: 0,
            staged: false,
            cancelled: false,
            compact: None,
        })
    }
    #[cfg(feature = "local-planning")]
    pub(crate) fn specialize_verified(
        verified: crate::appliance::assigned::VerifiedObservationPublication<'_>,
    ) -> Result<(), StructuredInfoRefusal> {
        let (producer, publication) = verified.into_parts();
        publication.specialize_fresh(producer)
    }
    /// Only a still-fresh actual publication can retire its generic buffers.
    /// Production enters through the opaque full Source/edge/owner witness.
    #[cfg(any(feature = "local-planning", test))]
    fn specialize_fresh(
        &mut self,
        _producer: &crate::observation::ObservationBack,
    ) -> Result<(), StructuredInfoRefusal> {
        if self.channel != PublicationChannel::Reading
            || self.sequence != 0
            || self.events != 0
            || self.staged
            || self.cancelled
            || self.length != 0
            || !self.current.is_empty()
            || self.compact.is_some()
        {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let layout = core::alloc::Layout::new::<super::source_reading::CompactReading>();
        // Reserve exact cold backing before retiring either generic buffer.
        // Failure keeps all original state/capacity and sequence ownership.
        let pointer =
            unsafe { alloc::alloc::alloc(layout) }.cast::<super::source_reading::CompactReading>();
        if pointer.is_null() {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        // SAFETY: exact aligned unique allocation, initialized before Box owns it.
        let compact = unsafe {
            pointer.write(super::source_reading::CompactReading::default());
            Box::from_raw(pointer)
        };
        self.compact = Some(compact);
        self.current = Vec::new();
        self.pending = Vec::new();
        Ok(())
    }
    /// Mutable payload heap backing; object/metadata and scoped projection
    /// scratch are separate. This inspection confers no profile authority.
    pub fn retained_value_storage_bytes(&self) -> usize {
        self.current.capacity()
            + self.pending.capacity()
            + self.compact.as_ref().map_or(0, |_| {
                core::mem::size_of::<super::source_reading::CompactReading>()
            })
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Reconstruct exactly the checked canonical prefix and retained node.
    /// The scratch buffer returns before the caller can suspend. A Pending
    /// sink may poll again; every poll sees the same committed value and order.
    /// ```compile_fail
    /// use conduit_environment_runtime::publication::SourcePublicationBack;
    /// fn leak(back: &SourcePublicationBack) -> Option<&[u8]> {
    ///     back.with_snapshot(|snapshot| snapshot.canonical)
    /// }
    /// ```
    #[inline(never)]
    pub fn with_snapshot<R>(
        &self,
        consume: impl FnOnce(PublicationSnapshot<'_>) -> R,
    ) -> Option<R> {
        if self.sequence == 0 {
            return None;
        }
        let schema = self.channel.linked_type_bytes();
        let mut bytes = [0; material::RESULT_MAXIMUM];
        let length = match &self.compact {
            Some(compact) => compact.current.as_ref()?.encode(&mut bytes).ok()?,
            None => {
                let length = schema.len() + self.current.len();
                bytes[..schema.len()].copy_from_slice(schema);
                bytes[schema.len()..length].copy_from_slice(&self.current);
                length
            }
        };
        Some(consume(PublicationSnapshot {
            channel: self.channel,
            sequence: self.sequence,
            canonical: &bytes[..length],
        }))
    }
    fn discard_staged(&mut self) {
        if self.staged {
            self.order.release();
            if let Some(compact) = &mut self.compact {
                compact.pending = None;
            }
            self.staged = false;
        }
    }
    pub fn maximum_events(&self) -> u32 {
        self.maximum_events
    }
}
impl<const P: usize> StepBack<P> for SourcePublicationBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.discard_staged();
        if self.cancelled {
            return fail(FailureCode::Cancelled, 1);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 1);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    fail(FailureCode::InvalidLifecycle, 1)
                } else {
                    StepOutcome::Complete
                }
            } else {
                StepOutcome::Await
            };
        };
        if self.events >= self.maximum_events {
            return fail(FailureCode::InvalidInput, 2);
        }
        let Some(bytes) = input.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 3);
        };
        if bytes.len() != reference.byte_len as usize || self.validator.validate(bytes).is_err() {
            return fail(FailureCode::InvalidInput, 4);
        }
        if io.consume_fuel(1).is_err() {
            return fail(FailureCode::InvalidLifecycle, 2);
        }
        let packed = if self.compact.is_some() {
            match super::source_reading::PackedReading::parse(bytes) {
                Ok(value) => Some(value),
                Err(_) => return fail(FailureCode::InvalidInput, 6),
            }
        } else {
            None
        };
        if !self.order.reserve() {
            return fail(FailureCode::InvalidInput, 5);
        }
        // Full canonical validation above checked the independently linked
        // Type and complete extent before removing only its immutable prefix.
        let node = &bytes[self.channel.linked_type_bytes().len()..];
        match &mut self.compact {
            Some(compact) => compact.pending = packed,
            None => self.pending[..node.len()].copy_from_slice(node),
        }
        self.length = node.len();
        if io.consume(PortId(0)).is_err() {
            self.order.release();
            return fail(FailureCode::InvalidLifecycle, 3);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged && !self.cancelled {
            match &mut self.compact {
                Some(compact) => compact.current = compact.pending.take(),
                None => {
                    self.current.clear();
                    self.current.extend_from_slice(&self.pending[..self.length]);
                }
            }
            self.sequence = self.order.commit();
            self.events += 1;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.discard_staged();
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl Drop for SourcePublicationBack {
    fn drop(&mut self) {
        self.discard_staged();
    }
}

#[allow(dead_code)]
pub(super) mod material {
    include!(concat!(env!("OUT_DIR"), "/observation-material.rs"));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constructor_refuses_unknown_schema_before_claiming_sequence_or_backing() {
        let mut contract = PublicationContract::prepare(PublicationChannel::Reading).unwrap();
        // Received/custom schema cannot replace the independently linked profile.
        contract.value_type =
            conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u8")).unwrap();
        let order = PublicationSequence::prepare(1);
        assert!(matches!(
            SourcePublicationBack::prepare(&contract, 1, order.clone()),
            Err(StructuredInfoRefusal::WrongType)
        ));
        assert_eq!(order.committed(), 0);
        let contract = PublicationContract::prepare(PublicationChannel::Reading).unwrap();
        assert!(SourcePublicationBack::prepare(&contract, 1, order).is_ok());
    }
}

#[cfg(test)]
#[path = "source_back_tests.rs"]
mod compact_tests;
