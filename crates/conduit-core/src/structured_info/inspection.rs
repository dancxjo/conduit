use alloc::string::String;
use alloc::vec::Vec;

use crate::{KindId, Observation, ObservationKind, SignId};

use super::{
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};

pub const MAXIMUM_STRUCTURED_INSPECTION_NODES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredInfoInspection {
    pub sign_id: SignId,
    pub value_kind: KindId,
    pub type_digest: [u8; 32],
    pub value_digest: [u8; 32],
    pub nodes: Vec<StructuredInfoInspectionNode>,
    pub omitted_nodes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredInfoInspectionNode {
    pub ordinal: u16,
    pub parent: Option<u16>,
    pub member: StructuredInfoInspectionMember,
    pub shape: StructuredInfoInspectionShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredInfoInspectionMember {
    Root,
    RecordField(String),
    CollectionItem(u16),
    VariantPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredInfoInspectionShape {
    Leaf { kind: KindId, byte_len: u32 },
    Collection { length: u16 },
    Record { schema: KindId, field_count: u16 },
    Variant { schema: KindId, active_tag: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredInfoInspectionRefusal {
    NotValueSign,
    ProfileMismatch,
    InvalidStructuredValue(StructuredInfoRefusal),
    NodeCountOverflow,
}

impl StructuredInfoInspection {
    pub fn from_sign(
        observation: &Observation,
        expected_type: &StructuredInfoType,
    ) -> Result<Self, StructuredInfoInspectionRefusal> {
        let value = match &observation.kind {
            ObservationKind::ValueProduced { value }
            | ObservationKind::ValueAccepted { value }
            | ObservationKind::ValuePresented { value } => value,
            _ => return Err(StructuredInfoInspectionRefusal::NotValueSign),
        };
        let expected_profile = expected_type
            .profile()
            .map_err(StructuredInfoInspectionRefusal::InvalidStructuredValue)?;
        if value.value_kind != *expected_profile.value_kind() {
            return Err(StructuredInfoInspectionRefusal::ProfileMismatch);
        }
        let structured = StructuredInfoValue::from_canonical_bytes(&value.encoded)
            .map_err(StructuredInfoInspectionRefusal::InvalidStructuredValue)?;
        if structured.value_type() != expected_type {
            return Err(StructuredInfoInspectionRefusal::ProfileMismatch);
        }

        let mut builder = InspectionBuilder::default();
        builder.visit(&structured, None, StructuredInfoInspectionMember::Root)?;
        Ok(Self {
            sign_id: observation.sign_id.clone(),
            value_kind: value.value_kind.clone(),
            type_digest: expected_type
                .semantic_digest()
                .map_err(StructuredInfoInspectionRefusal::InvalidStructuredValue)?,
            value_digest: structured
                .semantic_digest()
                .map_err(StructuredInfoInspectionRefusal::InvalidStructuredValue)?,
            nodes: builder.nodes,
            omitted_nodes: builder.omitted_nodes,
        })
    }
}

#[derive(Default)]
struct InspectionBuilder {
    nodes: Vec<StructuredInfoInspectionNode>,
    omitted_nodes: u16,
}

impl InspectionBuilder {
    fn visit(
        &mut self,
        value: &StructuredInfoValue,
        parent: Option<u16>,
        member: StructuredInfoInspectionMember,
    ) -> Result<(), StructuredInfoInspectionRefusal> {
        if self.nodes.len() == MAXIMUM_STRUCTURED_INSPECTION_NODES {
            self.omitted_nodes = self
                .omitted_nodes
                .checked_add(1)
                .ok_or(StructuredInfoInspectionRefusal::NodeCountOverflow)?;
            self.count_descendants(value)?;
            return Ok(());
        }
        let ordinal = u16::try_from(self.nodes.len())
            .map_err(|_| StructuredInfoInspectionRefusal::NodeCountOverflow)?;
        let shape = inspection_shape(value)?;
        self.nodes.push(StructuredInfoInspectionNode {
            ordinal,
            parent,
            member,
            shape,
        });
        match value.shape() {
            StructuredInfoValueShape::Leaf(_) => {}
            StructuredInfoValueShape::Collection(values) => {
                for (index, child) in values.iter().enumerate() {
                    self.visit(
                        child,
                        Some(ordinal),
                        StructuredInfoInspectionMember::CollectionItem(index as u16),
                    )?;
                }
            }
            StructuredInfoValueShape::Record(fields) => {
                for field in fields {
                    self.visit(
                        field.value(),
                        Some(ordinal),
                        StructuredInfoInspectionMember::RecordField(String::from(field.name())),
                    )?;
                }
            }
            StructuredInfoValueShape::Variant { payload, .. } => self.visit(
                payload,
                Some(ordinal),
                StructuredInfoInspectionMember::VariantPayload,
            )?,
        }
        Ok(())
    }

    fn count_descendants(
        &mut self,
        value: &StructuredInfoValue,
    ) -> Result<(), StructuredInfoInspectionRefusal> {
        let children: Vec<&StructuredInfoValue> = match value.shape() {
            StructuredInfoValueShape::Leaf(_) => Vec::new(),
            StructuredInfoValueShape::Collection(values) => values.iter().collect(),
            StructuredInfoValueShape::Record(fields) => {
                fields.iter().map(|field| field.value()).collect()
            }
            StructuredInfoValueShape::Variant { payload, .. } => alloc::vec![payload],
        };
        for child in children {
            self.omitted_nodes = self
                .omitted_nodes
                .checked_add(1)
                .ok_or(StructuredInfoInspectionRefusal::NodeCountOverflow)?;
            self.count_descendants(child)?;
        }
        Ok(())
    }
}

fn inspection_shape(
    value: &StructuredInfoValue,
) -> Result<StructuredInfoInspectionShape, StructuredInfoInspectionRefusal> {
    match (value.value_type().shape(), value.shape()) {
        (StructuredInfoTypeShape::Leaf(kind), StructuredInfoValueShape::Leaf(bytes)) => {
            Ok(StructuredInfoInspectionShape::Leaf {
                kind: kind.clone(),
                byte_len: u32::try_from(bytes.len())
                    .map_err(|_| StructuredInfoInspectionRefusal::NodeCountOverflow)?,
            })
        }
        (
            StructuredInfoTypeShape::Collection { length, .. },
            StructuredInfoValueShape::Collection(_),
        ) => Ok(StructuredInfoInspectionShape::Collection { length }),
        (
            StructuredInfoTypeShape::Record { schema, fields },
            StructuredInfoValueShape::Record(_),
        ) => Ok(StructuredInfoInspectionShape::Record {
            schema: schema.clone(),
            field_count: fields.len() as u16,
        }),
        (
            StructuredInfoTypeShape::Variant { schema, .. },
            StructuredInfoValueShape::Variant { tag, .. },
        ) => Ok(StructuredInfoInspectionShape::Variant {
            schema: schema.clone(),
            active_tag: String::from(tag),
        }),
        _ => Err(StructuredInfoInspectionRefusal::InvalidStructuredValue(
            StructuredInfoRefusal::WrongType,
        )),
    }
}
