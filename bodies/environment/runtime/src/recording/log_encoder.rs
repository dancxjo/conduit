//! Lean framed recording content, not a journal, storage grant or acknowledgement.
use super::*;
use alloc::{string::String, vec};
use conduit_core as c;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub const LOG_KIND: &str = "environment/encode-recording";
pub const LOG_RECORD_BYTES: usize = 200;
/// Producer-local acceptance time, never a claim of physical acquisition time.
pub const LOG_CLOCK: &str = "producer-acceptance-boot-ms@1";
pub const LOG_CONTEXT_BYTES: usize = 1536;
pub const LOG_CONTEXT_PART_BYTES: usize = 160;
const HEADER: usize = 40;
const DOMAIN: &str = "conduit.environment/recording-context@1";
/// Independently supplied current admitted producer scope. `now` measures local
/// acceptance in this exact Boot, NOT physical acquisition or remote reception.
#[derive(Clone, Copy)]
pub struct LogContext<'a> {
    pub body: &'a str,
    pub producer: RecordingContext<'a>,
    /// Independent recording stream identity; not the live publication sequence.
    pub stream: [u8; 32],
}
#[derive(Debug, PartialEq, Eq)]
pub enum LogRefusal {
    Context,
    Sequence,
    Observation(ObservationRefusal),
    Canonical(c::StructuredInfoRefusal),
}
impl From<c::StructuredInfoRefusal> for LogRefusal {
    fn from(e: c::StructuredInfoRefusal) -> Self {
        Self::Canonical(e)
    }
}
impl From<ObservationRefusal> for LogRefusal {
    fn from(e: ObservationRefusal) -> Self {
        Self::Observation(e)
    }
}
/// Exact bounded provenance bytes. No context or scope is inferred from samples.
pub struct PreparedLogContext {
    pub(super) bytes: [u8; LOG_CONTEXT_BYTES],
    pub(super) len: usize,
    pub(super) digest: [u8; 32],
}
impl PreparedLogContext {
    pub fn prepare(
        context: LogContext<'_>,
        contract: &ObservationContract,
        first_sequence: u64,
    ) -> Result<Self, LogRefusal> {
        let mut result = Self {
            bytes: [0; LOG_CONTEXT_BYTES],
            len: 0,
            digest: [0; 32],
        };
        result.push(&[1])?;
        for text in [
            context.body,
            context.producer.host.as_str(),
            context.producer.boot.as_str(),
            context.producer.plan.as_str(),
            context.producer.play.as_str(),
            PreparedObservationQuantityPacker::TEMPERATURE_FORM_ID,
            PreparedObservationQuantityPacker::HUMIDITY_FORM_ID,
            RecordingDispositionForm::IDENTITY,
            LOG_CLOCK,
        ] {
            if text.is_empty() || text.len() > 128 {
                return Err(LogRefusal::Context);
            }
            result.push(&[text.len() as u8])?;
            result.push(text.as_bytes())?;
        }
        if context.stream == [0; 32] || first_sequence == 0 {
            return Err(LogRefusal::Context);
        }
        result.push(&context.stream)?;
        result.push(&contract.observation_type().semantic_digest()?)?;
        result.push(&first_sequence.to_le_bytes())?;
        result.digest = c::semantic_digest(DOMAIN, result.as_bytes());
        Ok(result)
    }
    fn push(&mut self, bytes: &[u8]) -> Result<(), LogRefusal> {
        let end = self
            .len
            .checked_add(bytes.len())
            .ok_or(LogRefusal::Context)?;
        self.bytes
            .get_mut(self.len..end)
            .ok_or(LogRefusal::Context)?
            .copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn parts(&self) -> u16 {
        self.len.div_ceil(LOG_CONTEXT_PART_BYTES) as u16
    }
    /// No owned heap backing; inline Self must be charged separately.
    pub const fn retained_backing_bytes(&self) -> usize {
        0
    }
    pub(super) fn frame(&self, ordinal: u16, out: &mut [u8; LOG_RECORD_BYTES]) -> usize {
        let start = usize::from(ordinal) * LOG_CONTEXT_PART_BYTES;
        let end = (start + LOG_CONTEXT_PART_BYTES).min(self.len);
        out[0] = 1;
        out[1] = 1;
        out[2..34].copy_from_slice(&self.digest);
        out[34..36].copy_from_slice(&ordinal.to_le_bytes());
        out[36..38].copy_from_slice(&self.parts().to_le_bytes());
        out[38..40].copy_from_slice(&(self.len as u16).to_le_bytes());
        out[HEADER..HEADER + end - start].copy_from_slice(&self.bytes[start..end]);
        HEADER + end - start
    }
}
/// Installs only a standard semantic operation; it creates no initialized Back,
/// scope, flash provider, reservation or execution authority.
pub struct LogContract {
    kind: c::Kind,
}
impl LogContract {
    pub fn prepare(observation: &ObservationContract) -> Result<Self, c::StructuredInfoRefusal> {
        let input = observation
            .observation_type()
            .profile()?
            .value_kind()
            .clone();
        let output = c::kind_id(c::BYTES_INFO_ID);
        let port = |id, kind, direction| c::PortDescriptor {
            port_id: c::port_id(id),
            value_kind: kind,
            direction,
            temporal: c::PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        };
        let bounds = [
            (
                c::FrontValueLocation::Input(c::port_id("observation")),
                input.clone(),
                crate::canonical_storage::bytes(observation.observation_type(), 4096)? as u32,
            ),
            (
                c::FrontValueLocation::Output(c::port_id("record")),
                output.clone(),
                200,
            ),
        ];
        Ok(Self {
            kind: c::Kind {
                kind_id: c::kind_id(LOG_KIND),
                kind_contract_revision: "environment/encode-recording@1".into(),
                startup_parameters: vec![],
                shorthand: Some((c::port_id("observation"), c::port_id("record"))),
                inputs: vec![port("observation", input, c::PortDirection::Input)],
                outputs: vec![port("record", output, c::PortDirection::Output)],
                configuration: vec![],
                semantic_laws: vec![c::KindSemanticLaw::ValueContracts(
                    bounds
                        .into_iter()
                        .map(|(location, kind, bytes)| {
                            Ok(c::FrontValueContract {
                                location,
                                contract: c::CheckedValueContract::new(kind, bytes, vec![])
                                    .map_err(|_| {
                                        c::StructuredInfoRefusal::MalformedCanonicalEncoding
                                    })?,
                            })
                        })
                        .collect::<Result<_, c::StructuredInfoRefusal>>()?,
                )],
                limits: c::CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: 4096,
                },
            },
        })
    }
    pub fn kind(&self) -> &c::Kind {
        &self.kind
    }
    pub fn install(
        &self,
        startup: &mut conduit_plot::StartupCatalog,
        profiles: &mut conduit_plot::ProfileCatalog,
    ) -> Result<(), String> {
        startup.insert(conduit_plot::KindSignature {
            kind: LOG_KIND.into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(LOG_KIND, self.kind.checked_front())?;
        profiles
            .insert_kind(self.kind.clone())
            .map_err(|e| alloc::format!("recording contract: {e:?}"))?;
        Ok(())
    }
}
enum Staged {
    Context,
    Sample { time: u64 },
}
/// Context records precede every sample. Steps stage bytes only; kernel commit
/// advances context/sequence and consumes the sample. This does NOT mean media
/// commitment: the separate admitted append Back must commit these exact bytes.
pub struct RecordingLogBack {
    context: PreparedLogContext,
    quantities: PreparedObservationQuantityPacker,
    ordinal: u16,
    next: u64,
    remaining: u32,
    last_time: Option<u64>,
    now: RecordingClock,
    output: [u8; LOG_RECORD_BYTES],
    len: usize,
    staged: Option<Staged>,
    cancelled: bool,
}
impl RecordingLogBack {
    pub fn prepare(
        context: LogContext<'_>,
        contract: &ObservationContract,
        first: u64,
        maximum_samples: u32,
        now: RecordingClock,
    ) -> Result<Self, LogRefusal> {
        if first == 0
            || maximum_samples == 0
            || first.checked_add(u64::from(maximum_samples) - 1).is_none()
        {
            return Err(LogRefusal::Sequence);
        }
        Ok(Self {
            context: PreparedLogContext::prepare(context, contract, first)?,
            quantities: PreparedObservationQuantityPacker::prepare(contract)?,
            ordinal: 0,
            next: first,
            remaining: maximum_samples,
            last_time: None,
            now,
            output: [0; LOG_RECORD_BYTES],
            len: 0,
            staged: None,
            cancelled: false,
        })
    }
    pub const fn retained_backing_bytes(&self) -> usize {
        0
    }
    pub fn next_sequence(&self) -> u64 {
        self.next
    }
    pub fn committed_context_parts(&self) -> u16 {
        self.ordinal
    }
}
impl<const P: usize> StepBack<P> for RecordingLogBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = None;
        if self.cancelled {
            return fail(FailureCode::Cancelled, 1);
        }
        if self.remaining == 0 {
            return StepOutcome::Complete;
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 2);
        }
        let Some(value) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                StepOutcome::Complete
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = input.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 3);
        };
        if bytes.len() != value.byte_len as usize {
            return fail(FailureCode::InvalidInput, 4);
        }
        let Ok(packed) = self.quantities.pack(bytes) else {
            return fail(FailureCode::InvalidInput, 5);
        };
        if let Err(e) = io.consume_fuel(1) {
            return super::encoder::io_failure(e, 30);
        }
        let stage = if self.ordinal < self.context.parts() {
            self.len = self.context.frame(self.ordinal, &mut self.output);
            Staged::Context
        } else {
            let time = (self.now)();
            if self.last_time.is_some_and(|last| time < last) {
                return fail(FailureCode::InvalidInput, 6);
            }
            self.output[0] = 2;
            self.output[1] = 1;
            self.output[2..34].copy_from_slice(&self.context.digest);
            let mut sample = [0; SAMPLE_BYTES];
            sample[..8].copy_from_slice(&self.next.to_le_bytes());
            sample[8..16].copy_from_slice(&time.to_le_bytes());
            sample[16] = RecordingDispositionForm::encode(RecordingDisposition::Available)[0];
            sample[17..].copy_from_slice(&packed);
            self.output[34..55].copy_from_slice(&sample);
            self.len = 55;
            if let Err(e) = io.consume(PortId(0)) {
                return super::encoder::io_failure(e, 31);
            }
            Staged::Sample { time }
        };
        if let Err(e) = io.send_prepared(PortId(0), self.len as u32) {
            return super::encoder::io_failure(e, 32);
        }
        self.staged = Some(stage);
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged.is_some()).then_some(&self.output[..self.len])
    }
    fn step_committed(&mut self) {
        if self.cancelled {
            self.staged = None;
            return;
        }
        match self.staged.take() {
            Some(Staged::Context) => self.ordinal += 1,
            Some(Staged::Sample { time }) => {
                self.last_time = Some(time);
                self.remaining -= 1;
                if self.remaining != 0 {
                    self.next += 1
                }
            }
            None => {}
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = None;
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code,
        detail: 2400 + detail,
    })
}

#[cfg(test)]
mod tests;
