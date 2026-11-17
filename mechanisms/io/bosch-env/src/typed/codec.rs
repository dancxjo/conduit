use super::*;
use crate::bme280_observation::{compensate, Calibration, Error, Sample, Unavailable};
use alloc::vec::Vec;
use conduit_core::{
    BorrowedStructuredValue as V, PreparedLinkedStructuredComposer, StructuredInfoRefusal as R,
    ValidatedCanonicalStructuredValue as Value,
};
#[derive(Debug, PartialEq, Eq)]
pub enum CodecError {
    Canonical(R),
    Arithmetic,
}
impl From<R> for CodecError {
    fn from(value: R) -> Self {
        Self::Canonical(value)
    }
}

pub struct PreparedObservationCodec {
    input: BorrowedStructuredValueValidator<'static>,
    writer: PreparedLinkedStructuredComposer<'static>,
    output: Vec<u8>,
    length: usize,
}
impl PreparedObservationCodec {
    /// Allocate the one exact finite output buffer before execution.
    pub fn prepare(_contract: &ObservationContract) -> Result<Self, R> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(RESULT_MAXIMUM_BYTES)
            .map_err(|_| R::CanonicalEncodingTooLarge)?;
        output.resize(RESULT_MAXIMUM_BYTES, 0);
        Ok(Self {
            input: BorrowedStructuredValueValidator::new(INPUT_TYPE_BYTES, INPUT_MAXIMUM_BYTES)?,
            writer: PreparedLinkedStructuredComposer::new(RESULT_TYPE_BYTES, RESULT_MAXIMUM_BYTES)?,
            output,
            length: 0,
        })
    }
    /// Mutable heap backing only; inline Self and immutable linked schemas are
    /// separate charges. There are no owned tables or hidden nested buffers.
    pub fn retained_backing_bytes(&self) -> usize {
        self.output.capacity()
    }
    pub fn output(&self) -> &[u8] {
        &self.output[..self.length]
    }
    pub(super) fn clear(&mut self) {
        self.length = 0;
    }
    pub fn convert(&mut self, input: &[u8]) -> Result<&[u8], CodecError> {
        self.clear();
        let input = self.input.validate(input)?;
        let calibration = field(input, "calibration")?;
        let sample = field(input, "sample")?;
        let c = |name| integer(field(calibration, name)?);
        let s = |name| integer(field(sample, name)?);
        let result = compensate(
            Calibration {
                t1: c("t1")?,
                t2: c("t2")?,
                t3: c("t3")?,
                p1: c("p1")?,
                p2: c("p2")?,
                p3: c("p3")?,
                p4: c("p4")?,
                p5: c("p5")?,
                p6: c("p6")?,
                p7: c("p7")?,
                p8: c("p8")?,
                p9: c("p9")?,
                h1: c("h1")?,
                h2: c("h2")?,
                h3: c("h3")?,
                h4: c("h4")?,
                h5: c("h5")?,
                h6: c("h6")?,
            },
            Sample {
                pressure: s("pressure")?,
                temperature: s("temperature")?,
                humidity: s("humidity")?,
            },
        );
        self.length = match result {
            Ok(observation) => {
                let h = observation.humidity_q22_10_percent.to_le_bytes();
                let p = observation.pressure_q24_8_pa.to_le_bytes();
                let t = observation.temperature_centidegrees.to_le_bytes();
                let fields = [V::Leaf(&h), V::Leaf(&p), V::Leaf(&t)];
                self.writer
                    .encode_into(
                        V::Variant {
                            tag: "observation",
                            payload: &V::Record(&fields),
                        },
                        &mut self.output,
                    )?
                    .len()
            }
            Err(Error::Unavailable(failure)) => {
                let tag = match failure {
                    Unavailable::Malformed => "malformed",
                    Unavailable::SampleDisabled => "sample-disabled",
                    Unavailable::InvalidCalibration => "invalid-calibration",
                };
                let failure = V::Variant {
                    tag,
                    payload: &V::Empty,
                };
                self.writer
                    .encode_into(
                        V::Variant {
                            tag: "unavailable",
                            payload: &failure,
                        },
                        &mut self.output,
                    )?
                    .len()
            }
            Err(Error::Arithmetic) => return Err(CodecError::Arithmetic),
        };
        Ok(self.output())
    }
}
fn field<'a>(value: Value<'a>, name: &str) -> Result<Value<'a>, R> {
    value.record_field(name)?.ok_or(R::WrongRecordFields)
}
fn integer(value: Value<'_>) -> Result<i128, R> {
    let bytes = value.value_node();
    if bytes.get(..5) != Some(&[0, 16, 0, 0, 0]) {
        return Err(R::MalformedCanonicalEncoding);
    }
    Ok(i128::from_le_bytes(
        bytes[5..]
            .try_into()
            .map_err(|_| R::MalformedCanonicalEncoding)?,
    ))
}
