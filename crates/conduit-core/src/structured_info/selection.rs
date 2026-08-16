use alloc::string::String;
use alloc::vec::Vec;

use sha2::{Digest, Sha256};

use super::{
    validate_name, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeNode,
    StructuredInfoValue, StructuredInfoValueNode, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

const SELECTOR_DIGEST_DOMAIN: &[u8] = b"conduit.structured-info.selector.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnmatchedVariantDisposition {
    Drop,
    Refuse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StructuredSelectorOperation {
    Field(String),
    Index(u16),
    Variant {
        tag: String,
        unmatched: UnmatchedVariantDisposition,
    },
}

/// One immutable, statically checked operation over existing finite structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredSelector {
    input_type: StructuredInfoType,
    output_type: StructuredInfoType,
    operation: StructuredSelectorOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredSelectorRefusal {
    InvalidName(StructuredInfoRefusal),
    NotARecord,
    NotACollection,
    NotAVariant,
    UnknownField,
    IndexOutOfRange,
    UnknownVariantTag,
    WrongInputType,
    MalformedCheckedValue,
    UnmatchedVariant,
    FlowAlreadyClosed,
    CanonicalEncodingTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredSelection {
    Matched(StructuredInfoValue),
    Unmatched(UnmatchedVariantDisposition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredFlowSelection {
    Emitted(StructuredInfoValue),
    UnmatchedDropped,
    Pressure,
    Closed,
}

impl StructuredSelector {
    pub fn field(
        input_type: StructuredInfoType,
        field: impl Into<String>,
    ) -> Result<Self, StructuredSelectorRefusal> {
        let field = field.into();
        validate_name(&field).map_err(StructuredSelectorRefusal::InvalidName)?;
        let StructuredInfoTypeNode::Record { fields, .. } = &input_type.0 else {
            return Err(StructuredSelectorRefusal::NotARecord);
        };
        let output_type = fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|candidate| candidate.value_type.clone())
            .ok_or(StructuredSelectorRefusal::UnknownField)?;
        Ok(Self {
            input_type,
            output_type,
            operation: StructuredSelectorOperation::Field(field),
        })
    }

    pub fn index(
        input_type: StructuredInfoType,
        index: u16,
    ) -> Result<Self, StructuredSelectorRefusal> {
        let StructuredInfoTypeNode::Collection { element, length } = &input_type.0 else {
            return Err(StructuredSelectorRefusal::NotACollection);
        };
        if index >= *length {
            return Err(StructuredSelectorRefusal::IndexOutOfRange);
        }
        let output_type = element.as_ref().clone();
        Ok(Self {
            input_type,
            output_type,
            operation: StructuredSelectorOperation::Index(index),
        })
    }

    pub fn variant(
        input_type: StructuredInfoType,
        tag: impl Into<String>,
        unmatched: UnmatchedVariantDisposition,
    ) -> Result<Self, StructuredSelectorRefusal> {
        let tag = tag.into();
        validate_name(&tag).map_err(StructuredSelectorRefusal::InvalidName)?;
        let StructuredInfoTypeNode::Variant { cases, .. } = &input_type.0 else {
            return Err(StructuredSelectorRefusal::NotAVariant);
        };
        let output_type = cases
            .iter()
            .find(|candidate| candidate.tag == tag)
            .map(|candidate| candidate.payload_type.clone())
            .ok_or(StructuredSelectorRefusal::UnknownVariantTag)?;
        Ok(Self {
            input_type,
            output_type,
            operation: StructuredSelectorOperation::Variant { tag, unmatched },
        })
    }

    pub fn input_type(&self) -> &StructuredInfoType {
        &self.input_type
    }

    pub fn output_type(&self) -> &StructuredInfoType {
        &self.output_type
    }

    pub fn unmatched_disposition(&self) -> Option<UnmatchedVariantDisposition> {
        match self.operation {
            StructuredSelectorOperation::Variant { unmatched, .. } => Some(unmatched),
            StructuredSelectorOperation::Field(_) | StructuredSelectorOperation::Index(_) => None,
        }
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, StructuredSelectorRefusal> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(SELECTOR_DIGEST_DOMAIN);
        let input = self
            .input_type
            .canonical_bytes()
            .map_err(|_| StructuredSelectorRefusal::CanonicalEncodingTooLarge)?;
        push_bytes(&mut encoded, &input);
        match &self.operation {
            StructuredSelectorOperation::Field(field) => {
                encoded.push(0);
                push_bytes(&mut encoded, field.as_bytes());
            }
            StructuredSelectorOperation::Index(index) => {
                encoded.push(1);
                encoded.extend_from_slice(&index.to_le_bytes());
            }
            StructuredSelectorOperation::Variant { tag, unmatched } => {
                encoded.push(2);
                push_bytes(&mut encoded, tag.as_bytes());
                encoded.push(match unmatched {
                    UnmatchedVariantDisposition::Drop => 0,
                    UnmatchedVariantDisposition::Refuse => 1,
                });
            }
        }
        if encoded.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredSelectorRefusal::CanonicalEncodingTooLarge);
        }
        Ok(encoded)
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], StructuredSelectorRefusal> {
        let mut digest = Sha256::new();
        digest.update(self.canonical_bytes()?);
        Ok(digest.finalize().into())
    }

    pub fn select(
        &self,
        input: &StructuredInfoValue,
    ) -> Result<StructuredSelection, StructuredSelectorRefusal> {
        if input.value_type != self.input_type {
            return Err(StructuredSelectorRefusal::WrongInputType);
        }
        match (&self.operation, &input.node) {
            (
                StructuredSelectorOperation::Field(field),
                StructuredInfoValueNode::Record(fields),
            ) => fields
                .iter()
                .find(|candidate| candidate.name == *field)
                .map(|field| StructuredSelection::Matched(field.value.clone()))
                .ok_or(StructuredSelectorRefusal::MalformedCheckedValue),
            (
                StructuredSelectorOperation::Index(index),
                StructuredInfoValueNode::Collection(values),
            ) => values
                .get(usize::from(*index))
                .cloned()
                .map(StructuredSelection::Matched)
                .ok_or(StructuredSelectorRefusal::MalformedCheckedValue),
            (
                StructuredSelectorOperation::Variant { tag, unmatched },
                StructuredInfoValueNode::Variant {
                    tag: actual,
                    payload,
                },
            ) => {
                if actual == tag {
                    Ok(StructuredSelection::Matched(payload.as_ref().clone()))
                } else {
                    Ok(StructuredSelection::Unmatched(*unmatched))
                }
            }
            _ => Err(StructuredSelectorRefusal::MalformedCheckedValue),
        }
    }
}

/// Stateless finite flow adapter. The caller retains a pressured input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredFlowSelector {
    selector: StructuredSelector,
    closed: bool,
}

impl StructuredFlowSelector {
    pub fn new(selector: StructuredSelector) -> Self {
        Self {
            selector,
            closed: false,
        }
    }

    pub fn selector(&self) -> &StructuredSelector {
        &self.selector
    }

    pub fn offer(
        &mut self,
        input: &StructuredInfoValue,
        output_ready: bool,
    ) -> Result<StructuredFlowSelection, StructuredSelectorRefusal> {
        if self.closed {
            return Err(StructuredSelectorRefusal::FlowAlreadyClosed);
        }
        match self.selector.select(input)? {
            StructuredSelection::Matched(_) if !output_ready => {
                Ok(StructuredFlowSelection::Pressure)
            }
            StructuredSelection::Matched(value) => Ok(StructuredFlowSelection::Emitted(value)),
            StructuredSelection::Unmatched(UnmatchedVariantDisposition::Drop) => {
                Ok(StructuredFlowSelection::UnmatchedDropped)
            }
            StructuredSelection::Unmatched(UnmatchedVariantDisposition::Refuse) => {
                Err(StructuredSelectorRefusal::UnmatchedVariant)
            }
        }
    }

    pub fn close(&mut self) -> Result<StructuredFlowSelection, StructuredSelectorRefusal> {
        if self.closed {
            return Err(StructuredSelectorRefusal::FlowAlreadyClosed);
        }
        self.closed = true;
        Ok(StructuredFlowSelection::Closed)
    }
}

fn push_bytes(encoded: &mut Vec<u8>, bytes: &[u8]) {
    encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    encoded.extend_from_slice(bytes);
}
