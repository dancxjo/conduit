//! Snapshot publication through the ordinary Host Call Back. No store, scheduler,
//! retry, grant issuer or synthetic durable acknowledgement lives here.
use super::*;
use conduit_core::BoundedResourceRef;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedRange {
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub samples: usize,
}
use conduit_resource_snapshot::{SnapshotContentValidator, SnapshotPublicationBack};
pub type RecordingCommitObserver = conduit_resource_snapshot::SnapshotCommitObserver<RecordedRange>;
struct RecordingValidator {
    reader: PreparedRecordingReader,
    samples: usize,
}
impl SnapshotContentValidator for RecordingValidator {
    type Metadata = RecordedRange;
    fn validate(&mut self, bytes: &[u8]) -> Result<RecordedRange, Failure> {
        let json = core::str::from_utf8(bytes).map_err(|_| invalid(9))?;
        if self.reader.read(json).is_err() || self.reader.len() != self.samples {
            return Err(invalid(10));
        }
        Ok(RecordedRange {
            first_sequence: self
                .reader
                .sample(0)
                .expect("nonempty selected chunk")
                .sequence,
            last_sequence: self
                .reader
                .sample(self.samples - 1)
                .expect("selected sample count")
                .sequence,
            samples: self.samples,
        })
    }
}
pub struct RecordingSnapshotBack {
    inner: SnapshotPublicationBack<RecordingValidator>,
}
impl RecordingSnapshotBack {
    pub fn commit_observer(&self) -> RecordingCommitObserver {
        self.inner.commit_observer()
    }
    /// Root must install this under the exact resource snapshot publish contract,
    /// with its ordinary current placement, resource and independent authority.
    /// These content checks do not authorize the Host Call. One Back publishes
    /// at most one generation; additional flushes require additional admission.
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: usize,
        reference: &BoundedResourceRef,
    ) -> Result<(Self, RecordingCommitObserver), RecordingSnapshotRefusal> {
        let content_bytes = PreparedRecordingSnapshot::content_bytes(
            RecordingContext {
                host: context.host,
                boot: context.boot,
                plan: context.plan,
                play: context.play,
            },
            contract,
            samples,
        )?;
        let snapshot = PreparedRecordingSnapshot::prepare(
            RecordingContext {
                host: context.host,
                boot: context.boot,
                plan: context.plan,
                play: context.play,
            },
            contract,
            samples,
            reference,
        )?;
        let validator = RecordingValidator {
            reader: PreparedRecordingReader::prepare(context, contract)?,
            samples,
        };
        let (inner, observer) = SnapshotPublicationBack::prepare(
            validator,
            content_bytes,
            MAXIMUM_CHUNK_BYTES as u32,
            snapshot.expected_reference().to_vec(),
            "conduit.environment/recording-chunk@1",
        )
        .map_err(|_| RecordingSnapshotRefusal::Extent)?;
        Ok((Self { inner }, observer))
    }
}
impl<const P: usize> StepBack<P> for RecordingSnapshotBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.inner.step(io, input)
    }
    fn step_committed(&mut self) {
        <SnapshotPublicationBack<RecordingValidator> as StepBack<P>>::step_committed(
            &mut self.inner,
        )
    }
    fn cancel(&mut self) {
        <SnapshotPublicationBack<RecordingValidator> as StepBack<P>>::cancel(&mut self.inner)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        <SnapshotPublicationBack<RecordingValidator> as StepBack<P>>::prepared_output(
            &self.inner,
            port,
        )
    }
}

fn invalid(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail: 2200 + detail,
    }
}
