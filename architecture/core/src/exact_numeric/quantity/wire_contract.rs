//! Finite representation eligibility per port, distinct from physical meaning.
use super::QuantityValueProfile;
use crate::{
    kind_id, PortId, StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
};
use alloc::vec::Vec;

pub const QUANTITY_WIRE_CONTRACT_SCHEMA: &str = "numeric/quantity-wire-profile@1";
pub const MAXIMUM_QUANTITY_WIRE_PORTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuantityWireContract {
    inputs: Vec<(PortId, QuantityValueProfile)>,
    outputs: Vec<(PortId, QuantityValueProfile)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityWireContractRefusal {
    TooManyPorts,
    InvalidPort,
    DuplicatePort,
    InvalidProperty,
}
impl QuantityWireContract {
    pub fn new(
        mut inputs: Vec<(PortId, QuantityValueProfile)>,
        mut outputs: Vec<(PortId, QuantityValueProfile)>,
    ) -> Result<Self, QuantityWireContractRefusal> {
        if inputs.len() + outputs.len() > MAXIMUM_QUANTITY_WIRE_PORTS {
            return Err(QuantityWireContractRefusal::TooManyPorts);
        }
        for ports in [&mut inputs, &mut outputs] {
            ports.sort_by(|a, b| a.0.cmp(&b.0));
            if ports.iter().any(|(port, _)| {
                port.as_str().is_empty()
                    || port.as_str().len() > crate::MAXIMUM_STRUCTURED_NAME_BYTES
            }) {
                return Err(QuantityWireContractRefusal::InvalidPort);
            }
            if ports.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(QuantityWireContractRefusal::DuplicatePort);
            }
        }
        Ok(Self { inputs, outputs })
    }
    /// An omitted port retains its existing decimal-only wire contract. This
    /// default grants no optional exact representation capability.
    pub fn input(&self, port: &PortId) -> QuantityValueProfile {
        lookup(&self.inputs, port)
    }
    pub fn output(&self, port: &PortId) -> QuantityValueProfile {
        lookup(&self.outputs, port)
    }
    pub fn input_ports(&self) -> &[(PortId, QuantityValueProfile)] {
        &self.inputs
    }
    pub fn output_ports(&self) -> &[(PortId, QuantityValueProfile)] {
        &self.outputs
    }
    pub fn property_kind() -> crate::KindId {
        value_type()
            .expect("fixed finite wire contract Type")
            .profile()
            .expect("fixed finite wire contract profile")
            .value_kind()
            .clone()
    }
    pub fn to_property(&self) -> Result<StructuredConfigurationValue, QuantityWireContractRefusal> {
        let encode = || -> Result<_, StructuredInfoRefusal> {
            let ty = value_type()?;
            let inputs = entries(&self.inputs)?;
            let outputs = entries(&self.outputs)?;
            let value = StructuredInfoValue::record(
                ty,
                alloc::vec![
                    StructuredFieldValue::new("inputs", inputs)?,
                    StructuredFieldValue::new("outputs", outputs)?,
                ],
            )?;
            Ok((
                value.value_type().profile()?.value_kind().clone(),
                value.canonical_bytes()?,
            ))
        };
        let (profile, bytes) =
            encode().map_err(|_| QuantityWireContractRefusal::InvalidProperty)?;
        StructuredConfigurationValue::new(profile, bytes)
            .ok_or(QuantityWireContractRefusal::InvalidProperty)
    }
    pub fn from_property(
        property: &StructuredConfigurationValue,
    ) -> Result<Self, QuantityWireContractRefusal> {
        use QuantityWireContractRefusal::InvalidProperty as Invalid;
        if property.profile() != &Self::property_kind() {
            return Err(Invalid);
        }
        let value = StructuredInfoValue::from_canonical_bytes(property.canonical_value())
            .map_err(|_| Invalid)?;
        if value.value_type() != &value_type().map_err(|_| Invalid)? {
            return Err(Invalid);
        }
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            return Err(Invalid);
        };
        let [inputs, outputs] = fields else {
            return Err(Invalid);
        };
        let result = Self::new(
            decode_entries(inputs.value())?,
            decode_entries(outputs.value())?,
        )?;
        // Sorted unique entries are the one canonical domain encoding. A valid
        // structured sequence alone does not establish canonical port order.
        if result.to_property()?.canonical_value() != property.canonical_value() {
            return Err(Invalid);
        }
        Ok(result)
    }
}
fn lookup(ports: &[(PortId, QuantityValueProfile)], port: &PortId) -> QuantityValueProfile {
    ports
        .binary_search_by(|entry| entry.0.cmp(port))
        .ok()
        .map_or(QuantityValueProfile::Decimal, |index| ports[index].1)
}
fn entry_type() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    StructuredInfoType::record(
        kind_id("numeric/quantity-wire-port@1"),
        alloc::vec![
            StructuredFieldType::new(
                "port",
                StructuredInfoType::leaf(kind_id(crate::TEXT_INFO_ID))?
            )?,
            StructuredFieldType::new("profile", StructuredInfoType::leaf(kind_id("value/u8"))?)?,
        ],
    )
}
fn sequence_type() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    StructuredInfoType::sequence(entry_type()?, MAXIMUM_QUANTITY_WIRE_PORTS as u16)
}
fn value_type() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    StructuredInfoType::record(
        kind_id(QUANTITY_WIRE_CONTRACT_SCHEMA),
        alloc::vec![
            StructuredFieldType::new("inputs", sequence_type()?)?,
            StructuredFieldType::new("outputs", sequence_type()?)?,
        ],
    )
}
fn entries(
    ports: &[(PortId, QuantityValueProfile)],
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    let values = ports
        .iter()
        .map(|(port, profile)| {
            StructuredInfoValue::record(
                entry_type()?,
                alloc::vec![
                    StructuredFieldValue::new(
                        "port",
                        StructuredInfoValue::leaf(
                            StructuredInfoType::leaf(kind_id(crate::TEXT_INFO_ID))?,
                            port.as_str().as_bytes().to_vec()
                        )?
                    )?,
                    StructuredFieldValue::new(
                        "profile",
                        StructuredInfoValue::leaf(
                            StructuredInfoType::leaf(kind_id("value/u8"))?,
                            alloc::vec![match profile {
                                QuantityValueProfile::Decimal => 1,
                                QuantityValueProfile::Exact => 2,
                            }]
                        )?
                    )?,
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::sequence(sequence_type()?, values)
}
fn decode_entries(
    value: &StructuredInfoValue,
) -> Result<Vec<(PortId, QuantityValueProfile)>, QuantityWireContractRefusal> {
    use QuantityWireContractRefusal::InvalidProperty as Invalid;
    let StructuredInfoValueShape::Collection(entries) = value.shape() else {
        return Err(Invalid);
    };
    entries
        .iter()
        .map(|entry| {
            let StructuredInfoValueShape::Record(fields) = entry.shape() else {
                return Err(Invalid);
            };
            let [port, profile] = fields else {
                return Err(Invalid);
            };
            let StructuredInfoValueShape::Leaf(port) = port.value().shape() else {
                return Err(Invalid);
            };
            let StructuredInfoValueShape::Leaf(profile) = profile.value().shape() else {
                return Err(Invalid);
            };
            let profile = match profile {
                [1] => QuantityValueProfile::Decimal,
                [2] => QuantityValueProfile::Exact,
                _ => return Err(Invalid),
            };
            Ok((
                PortId::from(core::str::from_utf8(port).map_err(|_| Invalid)?),
                profile,
            ))
        })
        .collect()
}
