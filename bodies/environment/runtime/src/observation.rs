//! The authored Forms lift decoder coordinates into first-class quantities.
//! Checksum/range/negative decoding stays in the protocol Source.
use alloc::vec;
use conduit_core::{
    self as c, StructuredInfoRefusal as R, StructuredInfoType as Type,
    StructuredInfoTypeShape as Shape,
};
#[allow(dead_code)]
mod forms {
    include!(concat!(env!("OUT_DIR"), "/observation-forms.rs"));
}
mod back;
mod codec;
mod packed;
mod source_frame;
mod template;
pub use back::ObservationBack;
pub use codec::{PreparedObservationCodec, PreparedObservationEmitter};
pub use packed::PreparedObservationQuantityForms;
pub use source_frame::{PreparedSourceFrameDecoder, SourceFrameRefusal};
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
