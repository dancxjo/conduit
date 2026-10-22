//! The authored Forms lift decoder coordinates into first-class quantities.
//! Checksum/range/negative decoding stays in the protocol Source.
use alloc::vec;
use conduit_core::{
    self as c, validate_canonical_structured_value as validate,
    PreparedStructuredComposer as Composer, StructuredInfoRefusal as R, StructuredInfoType as Type,
    StructuredInfoTypeShape as Shape, ValidatedCanonicalStructuredValue as Value,
};
#[allow(dead_code)]
mod forms {
    include!(concat!(env!("OUT_DIR"), "/observation-forms.rs"));
}
mod back;
pub use back::ObservationBack;
pub const KIND: &str = "device/dht11-observation";
pub const MAXIMUM_BYTES: u32 = 4096;

pub struct ObservationContract {
    input: Type,
    observation: Type,
    output: Type,
    kind: c::Kind,
}
impl ObservationContract {
    pub fn prepare() -> Result<Self, R> {
        let input =
            Type::from_canonical_bytes(include_bytes!(concat!(env!("OUT_DIR"), "/decoded.type")))?;
        let observation = Type::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/observation.type"
        )))?;
        let output = Type::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/observation-result.type"
        )))?;
        let port = |name, ty: &Type, direction| -> Result<_, R> {
            Ok(c::PortDescriptor {
                port_id: c::port_id(name),
                value_kind: ty.profile()?.value_kind().clone(),
                direction,
                temporal: c::PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            })
        };
        let kind = c::Kind {
            kind_id: c::kind_id(KIND),
            kind_contract_revision: "device/dht11-observation@1".into(),
            startup_parameters: vec![],
            shorthand: Some((c::port_id("decoded"), c::port_id("result"))),
            inputs: vec![port("decoded", &input, c::PortDirection::Input)?],
            outputs: vec![port("result", &output, c::PortDirection::Output)?],
            configuration: vec![],
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAXIMUM_BYTES,
            },
        };
        Ok(Self {
            input,
            observation,
            output,
            kind,
        })
    }
    pub fn input_type(&self) -> &Type {
        &self.input
    }
    pub fn observation_type(&self) -> &Type {
        &self.observation
    }
    pub fn result_type(&self) -> &Type {
        &self.output
    }
    pub fn kind(&self) -> &c::Kind {
        &self.kind
    }
    pub fn install(
        &self,
        startup: &mut conduit_plot::StartupCatalog,
        profiles: &mut conduit_plot::ProfileCatalog,
    ) -> Result<(), alloc::string::String> {
        startup.insert(conduit_plot::KindSignature {
            kind: KIND.into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(KIND, self.kind.checked_front())?;
        profiles
            .insert_kind(self.kind.clone())
            .map_err(|r| alloc::format!("observation contract: {r:?}"))?;
        Ok(())
    }
}
fn field<'a>(ty: &'a Type, name: &str) -> Result<&'a Type, R> {
    let Shape::Record { fields, .. } = ty.shape() else {
        return Err(R::WrongType);
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value_type())
        .ok_or(R::WrongType)
}
#[derive(Debug, PartialEq, Eq)]
pub enum ObservationRefusal {
    Canonical(R),
    CoordinateOutOfRange,
    Form(c::QuantityFormRefusal),
}
impl From<R> for ObservationRefusal {
    fn from(r: R) -> Self {
        Self::Canonical(r)
    }
}
impl From<c::QuantityFormRefusal> for ObservationRefusal {
    fn from(r: c::QuantityFormRefusal) -> Self {
        Self::Form(r)
    }
}
pub struct PreparedObservationCodec {
    input: c::PreparedStructuredValueValidator,
    temperature: Composer,
    humidity: Composer,
    observation: Composer,
    result: Composer,
}
impl PreparedObservationCodec {
    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        let Shape::Record { fields, .. } = contract.observation.shape() else {
            return Err(R::WrongType);
        };
        if fields.len() != 2 || fields[0].name() != "humidity" || fields[1].name() != "temperature"
        {
            return Err(R::WrongRecordFields);
        }
        let temperature_type = field(&contract.observation, "temperature")?;
        let humidity_type = field(&contract.observation, "humidity")?;
        let leaf_bytes = |ty: &Type| -> Result<usize, R> {
            let bytes = ty.canonical_bytes()?.len() + 5 + c::QUANTITY_ENCODED_LEN;
            if bytes > MAXIMUM_BYTES as usize {
                return Err(R::CanonicalEncodingTooLarge);
            };
            Ok(bytes)
        };
        Ok(Self {
            input: c::PreparedStructuredValueValidator::new(&contract.input, 1024)?,
            temperature: Composer::new(temperature_type, leaf_bytes(temperature_type)?)?,
            humidity: Composer::new(humidity_type, leaf_bytes(humidity_type)?)?,
            observation: Composer::new(&contract.observation, MAXIMUM_BYTES as usize)?,
            result: Composer::new(&contract.output, MAXIMUM_BYTES as usize)?,
        })
    }
    pub fn convert(&mut self, input: &[u8]) -> Result<&[u8], ObservationRefusal> {
        self.input.validate(input)?;
        let decoded = validate(input)?;
        if let Some(frame) = decoded.variant_payload("frame")? {
            let temperature = i16::try_from(coordinate(
                frame.record_field("temperature")?.ok_or(R::WrongType)?,
            )?)
            .map_err(|_| ObservationRefusal::CoordinateOutOfRange)?;
            let humidity = u16::try_from(coordinate(
                frame.record_field("humidity")?.ok_or(R::WrongType)?,
            )?)
            .map_err(|_| ObservationRefusal::CoordinateOutOfRange)?;
            let temperature =
                forms::DeviceDht11TemperatureForm::decode(&temperature.to_le_bytes())?;
            let humidity = forms::DeviceDht11HumidityForm::decode(&humidity.to_le_bytes())?;
            self.temperature.leaf(&temperature.encode())?;
            self.humidity.leaf(&humidity.encode())?;
            self.observation.record(&[
                // Structured records use canonical field order, independent
                // of the order of fields in the authored declaration.
                validate(self.humidity.encoded())?,
                validate(self.temperature.encoded())?,
            ])?;
            self.result
                .variant("observation", validate(self.observation.encoded())?)?;
        } else {
            self.result.variant(
                "unavailable",
                decoded
                    .variant_payload("unavailable")?
                    .ok_or(R::WrongType)?,
            )?;
        }
        Ok(self.result.encoded())
    }
    pub fn output(&self) -> &[u8] {
        self.result.encoded()
    }
}
fn coordinate(value: Value<'_>) -> Result<i128, R> {
    let bytes = value.value_node();
    if bytes.get(..5) != Some(&[0, 16, 0, 0, 0]) {
        return Err(R::MalformedCanonicalEncoding);
    };
    Ok(i128::from_le_bytes(
        bytes[5..]
            .try_into()
            .map_err(|_| R::MalformedCanonicalEncoding)?,
    ))
}
