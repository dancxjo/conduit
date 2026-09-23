use super::{
    StructuredInfoType, StructuredInfoTypeNode, StructuredInfoValue, StructuredSelector,
    StructuredSelectorBack, StructuredSelectorRefusal, UnmatchedVariantDisposition,
};
use crate::structured_info::{validate_name, MAXIMUM_STRUCTURED_RECORD_FIELDS};
use alloc::{string::String, vec::Vec};

impl StructuredSelector {
    pub fn record_field_values(
        input_type: StructuredInfoType,
        field: impl Into<String>,
        mut expected: Vec<Vec<u8>>,
        match_when_present: bool,
        unmatched: UnmatchedVariantDisposition,
    ) -> Result<Self, StructuredSelectorRefusal> {
        let field = field.into();
        validate_name(&field).map_err(StructuredSelectorRefusal::InvalidName)?;
        if expected.is_empty() {
            return Err(StructuredSelectorRefusal::EmptyPredicateSet);
        }
        if expected.len() > MAXIMUM_STRUCTURED_RECORD_FIELDS {
            return Err(StructuredSelectorRefusal::TooManyPredicateValues);
        }
        let StructuredInfoTypeNode::Record { fields, .. } = &input_type.0 else {
            return Err(StructuredSelectorRefusal::NotARecord);
        };
        let value_type = fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|candidate| candidate.value_type.clone())
            .ok_or(StructuredSelectorRefusal::UnknownField)?;
        if !matches!(value_type.0, StructuredInfoTypeNode::Leaf(_)) {
            return Err(StructuredSelectorRefusal::PredicateRequiresLeafField);
        }
        for value in &expected {
            StructuredInfoValue::leaf(value_type.clone(), value.clone())
                .map_err(|_| StructuredSelectorRefusal::MalformedCheckedValue)?;
        }
        expected.sort();
        if expected.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(StructuredSelectorRefusal::DuplicatePredicateValue);
        }
        Ok(Self {
            output_type: input_type.clone(),
            input_type,
            operation: StructuredSelectorBack::RecordFieldValues {
                field,
                expected,
                match_when_present,
                unmatched,
            },
        })
    }
}
