//! Ordinary typed startup custody for the retained computation graph.
use super::*;
use crate::{
    kind_id, StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
    BYTES_INFO_ID,
};
use alloc::vec;

pub fn numeric_computation_info_type() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    let bytes = StructuredInfoType::leaf(kind_id(BYTES_INFO_ID))?;
    StructuredInfoType::record(
        kind_id(NUMERIC_COMPUTATION_INFO_ID),
        vec![
            StructuredFieldType::new("payload", bytes.clone())?,
            StructuredFieldType::new("program_identity", bytes)?,
        ],
    )
}
impl NumericComputation {
    pub fn configuration(
        &self,
        program_identity: [u8; 32],
    ) -> Result<StructuredConfigurationValue, NumericComputationRefusal> {
        let invalid = |_| NumericComputationRefusal::MalformedEncoding;
        let ty = numeric_computation_info_type().map_err(invalid)?;
        let bytes = StructuredInfoType::leaf(kind_id(BYTES_INFO_ID)).map_err(invalid)?;
        let value = StructuredInfoValue::record(
            ty,
            vec![
                StructuredFieldValue::new(
                    "payload",
                    StructuredInfoValue::leaf(bytes.clone(), self.canonical_bytes()?)
                        .map_err(invalid)?,
                )
                .map_err(invalid)?,
                StructuredFieldValue::new(
                    "program_identity",
                    StructuredInfoValue::leaf(bytes, program_identity.to_vec()).map_err(invalid)?,
                )
                .map_err(invalid)?,
            ],
        )
        .map_err(invalid)?;
        let profile = value
            .value_type()
            .profile()
            .map_err(invalid)?
            .value_kind()
            .clone();
        StructuredConfigurationValue::new(profile, value.canonical_bytes().map_err(invalid)?)
            .ok_or(NumericComputationRefusal::MalformedEncoding)
    }
    pub fn from_configuration(
        value: &StructuredConfigurationValue,
        program_identity: [u8; 32],
    ) -> Result<Self, NumericComputationRefusal> {
        let invalid = |_| NumericComputationRefusal::MalformedEncoding;
        let ty = numeric_computation_info_type().map_err(invalid)?;
        let actual =
            StructuredInfoValue::from_canonical_bytes(value.canonical_value()).map_err(invalid)?;
        if actual.value_type() != &ty
            || value.profile() != ty.profile().map_err(invalid)?.value_kind()
        {
            return Err(NumericComputationRefusal::MalformedEncoding);
        }
        let StructuredInfoValueShape::Record(fields) = actual.shape() else {
            return Err(NumericComputationRefusal::MalformedEncoding);
        };
        let leaf = |name: &str| {
            fields
                .iter()
                .find(|field| field.name() == name)
                .and_then(|field| match field.value().shape() {
                    StructuredInfoValueShape::Leaf(bytes) => Some(bytes),
                    _ => None,
                })
        };
        if leaf("program_identity") != Some(program_identity.as_slice()) {
            return Err(NumericComputationRefusal::MalformedEncoding);
        }
        Self::decode_validate(leaf("payload").ok_or(NumericComputationRefusal::MalformedEncoding)?)
    }
}
