#![no_std]
//! The authored Forms lift decoder coordinates into first-class quantities.
//! Checksum/range/negative decoding stays in the protocol Source.
extern crate alloc;
use alloc::vec;
use conduit_core::{
    self as c, StructuredInfoRefusal as R, StructuredInfoType as Type,
    StructuredInfoTypeShape as Shape,
};
#[allow(dead_code)]
mod forms {
    include!(concat!(env!("OUT_DIR"), "/observation-forms.rs"));
}
#[cfg(feature = "kernel-step")]
mod back;
mod observation_codec;
mod packed;
pub use observation_codec::PreparedObservationCodec;
mod source_frame;
mod storage_ceiling;
#[cfg(feature = "kernel-step")]
pub use back::ObservationBack;
pub use packed::{PreparedObservationQuantityForms, PreparedObservationQuantityPacker};
pub use source_frame::{PreparedSourceFrameDecoder, SourceFrameRefusal};
pub const KIND: &str = "device/dht11-observation";
pub const MAXIMUM_BYTES: u32 = 4096;
/// Immutable checked result schema, shared by typed publication consumers.
pub static RESULT_TYPE_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/observation-result.type"));
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
        let output = Type::from_canonical_bytes(RESULT_TYPE_BYTES)?;
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
            kind_contract_revision: "device/dht11-observation@2".into(),
            startup_parameters: vec![],
            shorthand: Some((c::port_id("decoded"), c::port_id("result"))),
            inputs: vec![port("decoded", &input, c::PortDirection::Input)?],
            outputs: vec![port("result", &output, c::PortDirection::Output)?],
            configuration: vec![],
            semantic_laws: vec![c::KindSemanticLaw::ValueContracts(vec![
                c::FrontValueContract {
                    location: c::FrontValueLocation::Input(c::port_id("decoded")),
                    contract: c::CheckedValueContract::new(
                        input.profile()?.value_kind().clone(),
                        storage_ceiling::bytes(&input, 1024)? as u32,
                        vec![],
                    )
                    .map_err(|_| R::MalformedCanonicalEncoding)?,
                },
                c::FrontValueContract {
                    location: c::FrontValueLocation::Output(c::port_id("result")),
                    contract: c::CheckedValueContract::new(
                        output.profile()?.value_kind().clone(),
                        storage_ceiling::bytes(&output, MAXIMUM_BYTES as usize)? as u32,
                        vec![],
                    )
                    .map_err(|_| R::MalformedCanonicalEncoding)?,
                },
            ])],
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linked_observation_codecs_refuse_foreign_contract_types() {
        let mut contract = ObservationContract::prepare().unwrap();
        contract.input = contract.observation.clone();
        assert!(matches!(
            PreparedObservationCodec::prepare(&contract),
            Err(R::WrongType)
        ));
        let mut contract = ObservationContract::prepare().unwrap();
        contract.observation = contract.input.clone();
        assert!(matches!(
            PreparedObservationQuantityForms::prepare(&contract),
            Err(R::WrongType)
        ));
    }
}
