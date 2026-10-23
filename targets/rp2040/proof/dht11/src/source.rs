//! Executes the checked Source expression; no native checksum/decoder fork.
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as Composer,
    Quantity, StructuredInfoType, StructuredInfoTypeShape,
    ValidatedCanonicalStructuredValue as Value,
};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

#[allow(dead_code)]
mod forms {
    include!(concat!(env!("OUT_DIR"), "/forms.rs"));
}

pub struct SourceDecoder {
    evaluator: PreparedPortableExpressionEvaluator,
    octets: [Composer; 5],
    length: Composer,
    wire: Composer,
    capture: Composer,
    #[cfg(feature = "pio-capture")]
    pub pulse_request: conduit_pulse_capture::CheckedPulseRead,
}
pub type Reading = Result<(Quantity, Quantity), &'static str>;

fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("checked record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
impl SourceDecoder {
    /// Target ABI/Form and borrowed-composition checks after allocation is sealed.
    pub fn check_adapter(&mut self) {
        let (temperature, humidity) = self.evaluate(5, [53, 0, 24, 0, 77]).unwrap();
        assert_eq!((temperature.coefficient(), temperature.exponent()), (24, 0));
        assert_eq!((humidity.coefficient(), humidity.exponent()), (53, 0));
        let (temperature, _) = self.evaluate(5, [53, 0, 10, 130, 193]).unwrap();
        assert_eq!(
            (temperature.coefficient(), temperature.exponent()),
            (-108, -1)
        );
        for (length, wire, expected) in [
            (2, [0; 5], "short-frame"),
            (5, [53, 0, 24, 0, 76], "checksum"),
            (5, [0; 5], "invalid-payload"),
        ] {
            assert!(matches!(self.evaluate(length, wire), Err(actual) if actual == expected));
        }
    }
    pub fn prepare() -> Self {
        #[cfg(feature = "pio-capture")]
        let pulse_request = prepare_pulse_request();
        let program = PortableExpressionProgram::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/dht11.program"
        )))
        .unwrap();
        let ty = &program.input_type;
        let wire_type = field(ty, "wire");
        let StructuredInfoTypeShape::Collection { element, length: 5 } = wire_type.shape() else {
            panic!("five octets")
        };
        Self {
            evaluator: PreparedPortableExpressionEvaluator::new(&program).unwrap(),
            octets: core::array::from_fn(|_| Composer::new(element, 64).unwrap()),
            length: Composer::new(field(ty, "length"), 64).unwrap(),
            wire: Composer::new(wire_type, 256).unwrap(),
            capture: Composer::new(ty, 512).unwrap(),
            #[cfg(feature = "pio-capture")]
            pulse_request,
        }
    }
    pub fn evaluate(&mut self, length: u8, wire: [u8; 5]) -> Reading {
        for (encoder, octet) in self.octets.iter_mut().zip(wire) {
            encoder.leaf(&[octet]).unwrap();
        }
        let octets: [_; 5] = core::array::from_fn(|i| validate(self.octets[i].encoded()).unwrap());
        self.wire.collection(&octets).unwrap();
        self.length.leaf(&[length]).unwrap();
        self.capture
            .record(&[
                validate(self.length.encoded()).unwrap(),
                validate(self.wire.encoded()).unwrap(),
            ])
            .unwrap();
        let result = validate(self.evaluator.evaluate(self.capture.encoded()).unwrap()).unwrap();
        if let Some(frame) = result.variant_payload("frame").unwrap() {
            let temperature = coordinate(frame.record_field("temperature").unwrap().unwrap());
            let humidity = coordinate(frame.record_field("humidity").unwrap().unwrap());
            Ok((
                forms::DeviceDht11TemperatureForm::decode(
                    &i16::try_from(temperature).unwrap().to_le_bytes(),
                )
                .unwrap(),
                forms::DeviceDht11HumidityForm::decode(
                    &u16::try_from(humidity).unwrap().to_le_bytes(),
                )
                .unwrap(),
            ))
        } else {
            let failure = result.variant_payload("unavailable").unwrap().unwrap();
            for tag in [
                "short-frame",
                "malformed-length",
                "checksum",
                "invalid-payload",
            ] {
                if failure.variant_payload(tag).unwrap().is_some() {
                    return Err(tag);
                }
            }
            panic!("checked failure variant");
        }
    }
}

#[cfg(feature = "pio-capture")]
fn prepare_pulse_request() -> conduit_pulse_capture::CheckedPulseRead {
    use conduit_pulse_capture::{PreparedPulseCodec, PulseReadContract};
    let program = PortableExpressionProgram::from_canonical_bytes(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/capture.program"
    )))
    .unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let contract = PulseReadContract::prepare().unwrap();
    let codec = PreparedPulseCodec::prepare(&contract).unwrap();
    codec
        .request(
            evaluator.evaluate(&[]).unwrap(),
            conduit_rp2040_pulse_capture::LIMITS,
        )
        .unwrap()
}

fn coordinate(value: Value<'_>) -> i128 {
    // The checked Source output is I128; canonical leaf framing is validated.
    let bytes = value.value_node();
    assert_eq!(&bytes[..5], &[0, 16, 0, 0, 0]);
    i128::from_le_bytes(bytes[5..].try_into().unwrap())
}
