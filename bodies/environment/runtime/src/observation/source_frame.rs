//! Pure frame conversion through the existing checked Source program and
//! quantity Forms. No pin, scheduler, publication, Host offer or authority.
use super::{field, ObservationContract, ObservationRefusal, PreparedObservationCodec};
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as C,
    StructuredInfoRefusal as R, StructuredInfoTypeShape as S,
};
use conduit_plot::{
    PortableExpressionEvaluationRefusal, PortableExpressionProgram,
    PortableExpressionProgramRefusal, PreparedPortableExpressionEvaluator,
};
#[derive(Debug, PartialEq, Eq)]
pub enum SourceFrameRefusal {
    #[cfg(feature = "source-capture")]
    Capture(conduit_pulse_capture::PulseCodecRefusal),
    Program(PortableExpressionProgramRefusal),
    Evaluation(PortableExpressionEvaluationRefusal),
    Canonical(R),
    Observation(ObservationRefusal),
}
impl From<R> for SourceFrameRefusal {
    fn from(value: R) -> Self {
        Self::Canonical(value)
    }
}
impl From<PortableExpressionEvaluationRefusal> for SourceFrameRefusal {
    fn from(value: PortableExpressionEvaluationRefusal) -> Self {
        Self::Evaluation(value)
    }
}
impl From<ObservationRefusal> for SourceFrameRefusal {
    fn from(value: ObservationRefusal) -> Self {
        Self::Observation(value)
    }
}
pub struct PreparedSourceFrameDecoder {
    evaluator: PreparedPortableExpressionEvaluator,
    octets: [C; 5],
    length: C,
    wire: C,
    capture: C,
    observation: PreparedObservationCodec,
}
mod identity {
    include!(concat!(env!("OUT_DIR"), "/dht11-source-identity.rs"));
}
impl PreparedSourceFrameDecoder {
    pub const SOURCE_DOCUMENT_ID: &'static str = identity::SOURCE_DOCUMENT_ID;
    pub const CHECKED_PLOT_ID: &'static str = identity::CHECKED_PLOT_ID;
    pub const EXPANDED_PLOT_ID: &'static str = identity::EXPANDED_PLOT_ID;
    /// Prepare electrical geometry from the checked request Plot. This is data,
    /// not a selected pin, a provider offer or permission to perform capture.
    #[cfg(feature = "source-capture")]
    pub fn capture_request(
        limits: conduit_pulse_capture::PulseReadLimits,
    ) -> Result<conduit_pulse_capture::CheckedPulseRead, SourceFrameRefusal> {
        let program = PortableExpressionProgram::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/dht11-capture.program"
        )))
        .map_err(SourceFrameRefusal::Program)?;
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program)?;
        let contract = conduit_pulse_capture::PulseReadContract::prepare()?;
        let codec = conduit_pulse_capture::PreparedPulseCodec::prepare(&contract)?;
        codec
            .request(evaluator.evaluate(&[])?, limits)
            .map_err(SourceFrameRefusal::Capture)
    }
    pub fn prepare(contract: &ObservationContract) -> Result<Self, SourceFrameRefusal> {
        let program = PortableExpressionProgram::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/dht11-decode.program"
        )))
        .map_err(SourceFrameRefusal::Program)?;
        if &program.output_type != contract.input_type() {
            return Err(R::WrongType.into());
        }
        let input = &program.input_type;
        let wire = field(input, "wire")?;
        let S::Collection { element, length: 5 } = wire.shape() else {
            return Err(R::WrongType.into());
        };
        let octet = || C::new(element, 64);
        Ok(Self {
            evaluator: PreparedPortableExpressionEvaluator::new(&program)?,
            octets: [octet()?, octet()?, octet()?, octet()?, octet()?],
            length: C::new(field(input, "length")?, 64)?,
            wire: C::new(wire, 256)?,
            capture: C::new(input, 512)?,
            observation: PreparedObservationCodec::prepare(contract)?,
        })
    }
    /// Length is never replaced with the padded array's extent. Source owns
    /// short/malformed lengths, checksum, range, sign and decimal interpretation.
    /// Returned canonical results retain Temperature/Ratio or typed unavailability.
    pub fn decode(&mut self, length: u8, wire: [u8; 5]) -> Result<&[u8], SourceFrameRefusal> {
        for (encoder, octet) in self.octets.iter_mut().zip(wire) {
            encoder.leaf(&[octet])?;
        }
        self.wire.collection(&[
            validate(self.octets[0].encoded())?,
            validate(self.octets[1].encoded())?,
            validate(self.octets[2].encoded())?,
            validate(self.octets[3].encoded())?,
            validate(self.octets[4].encoded())?,
        ])?;
        self.length.leaf(&[length])?;
        self.capture.record(&[
            validate(self.length.encoded())?,
            validate(self.wire.encoded())?,
        ])?;
        let decoded = self.evaluator.evaluate(self.capture.encoded())?;
        Ok(self.observation.convert(decoded)?)
    }
}
