//! The application's channel meaning wraps shared finite typed retention.
use super::{PublicationChannel, PublicationContract, PublicationSequence};
use conduit_core::StructuredInfoRefusal;
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_typed_retention::PreparedTypedRetention;

pub struct PublicationSnapshot<'a> {
    pub channel: PublicationChannel,
    pub sequence: u64,
    pub canonical: &'a [u8],
}
pub struct PublicationBack {
    channel: PublicationChannel,
    retained: PreparedTypedRetention<'static>,
}
impl PublicationBack {
    pub fn prepare(
        contract: &PublicationContract,
        maximum_events: u32,
        order: PublicationSequence,
    ) -> Result<Self, StructuredInfoRefusal> {
        let schema = contract.channel().linked_type_bytes();
        // Independently linked bytes must describe the exact prepared contract.
        // This comparison is transient preparation, not retained schema backing.
        if contract.value_type().canonical_bytes()?.as_slice() != schema {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let bytes = crate::canonical_storage::bytes(
            contract.value_type(),
            contract.channel().maximum_bytes(),
        )?;
        Ok(Self {
            channel: contract.channel(),
            retained: PreparedTypedRetention::prepare(schema, bytes, maximum_events, order)?,
        })
    }
    pub fn snapshot(&self) -> Option<PublicationSnapshot<'_>> {
        self.retained.snapshot().map(|value| PublicationSnapshot {
            channel: self.channel,
            sequence: value.sequence,
            canonical: value.canonical,
        })
    }
    pub fn maximum_events(&self) -> u32 {
        self.retained.maximum_events()
    }
    /// Two value buffers only; separately fund inline Self, shared order,
    /// schema residence and allocator metadata.
    pub fn value_storage_bytes(&self) -> Option<usize> {
        self.retained.value_storage_bytes()
    }
}
impl<const P: usize> StepBack<P> for PublicationBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.retained.step(io, input)
    }
    fn step_committed(&mut self) {
        <PreparedTypedRetention<'static> as StepBack<P>>::step_committed(&mut self.retained);
    }
    fn cancel(&mut self) {
        <PreparedTypedRetention<'static> as StepBack<P>>::cancel(&mut self.retained);
    }
}
