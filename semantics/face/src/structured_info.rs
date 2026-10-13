//! Redacted Face projection of one exact structured value Sign.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    Observation, StructuredInfoInspection, StructuredInfoInspectionMember,
    StructuredInfoInspectionRefusal, StructuredInfoInspectionShape, StructuredInfoLeafSemantic,
    StructuredInfoType,
};

use crate::{
    Face, FaceAspect, FaceBasis, FaceDepth, FaceError, FaceNavigation, FacePlace, FaceProjection,
    FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    NavigationAspect, NavigationPlace, NavigationRefusal, ProjectionItem, ProjectionMembership,
    ProjectionRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredSignFace {
    pub inspection: StructuredInfoInspection,
    pub face: Face,
    pub navigation: FaceNavigation,
    pub projection: FaceProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredSignFaceRefusal {
    InvalidDataLabel,
    Inspection(StructuredInfoInspectionRefusal),
    Face(FaceError),
    Navigation(NavigationRefusal),
    Projection(ProjectionRefusal),
}

impl StructuredSignFace {
    pub fn from_sign(
        revision: u64,
        observation: &Observation,
        expected_type: &StructuredInfoType,
    ) -> Result<Self, StructuredSignFaceRefusal> {
        if let conduit_core::ObservationKind::ValuePresented { label, .. } = &observation.kind {
            if !conduit_core::valid_data_label(label) {
                return Err(StructuredSignFaceRefusal::InvalidDataLabel);
            }
        }
        let inspection = StructuredInfoInspection::from_sign(observation, expected_type)
            .map_err(StructuredSignFaceRefusal::Inspection)?;
        let mut subjects = Vec::with_capacity(inspection.nodes.len());
        let mut relationships = Vec::with_capacity(inspection.nodes.len().saturating_sub(1));
        let mut properties = Vec::new();
        let mut subject_ids = Vec::with_capacity(inspection.nodes.len());

        for node in &inspection.nodes {
            let identity = node_identity(&inspection, node.ordinal);
            let label = if node.parent.is_none() {
                match &observation.kind {
                    conduit_core::ObservationKind::ValuePresented { label, .. } => label.clone(),
                    _ => node_label(node),
                }
            } else {
                node_label(node)
            };
            subjects.push(FaceSubject {
                identity: identity.clone(),
                role: if node.parent.is_none() {
                    FaceRole::Sign
                } else {
                    FaceRole::Info
                },
                name: label,
            });
            subject_ids.push(identity.clone());
            if let Some(parent) = node.parent {
                relationships.push(FaceRelationship {
                    source: subject_ids[usize::from(parent)].clone(),
                    target: identity.clone(),
                    kind: FaceRelationshipKind::Contains,
                });
            }
            add_node_properties(&mut properties, &identity, node);
        }
        let root = subject_ids
            .first()
            .expect("a valid structured value always has a root")
            .clone();
        properties.extend([
            property(
                &root,
                "value-kind",
                FacePropertyValue::Identity(inspection.value_kind.as_str().to_string()),
            ),
            property(
                &root,
                "type-digest",
                FacePropertyValue::Identity(hex(&inspection.type_digest)),
            ),
            property(
                &root,
                "value-digest",
                FacePropertyValue::Identity(hex(&inspection.value_digest)),
            ),
            property(
                &root,
                "omitted-node-count",
                FacePropertyValue::Count(u64::from(inspection.omitted_nodes)),
            ),
            property(
                &root,
                "leaf-content-redacted",
                FacePropertyValue::Flag(true),
            ),
        ]);

        let face = Face::new(
            revision,
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![inspection.sign_id.clone()],
            },
            subjects,
            relationships,
            properties,
            Vec::new(),
        )
        .map_err(StructuredSignFaceRefusal::Face)?;
        let navigation = FaceNavigation::new(
            &face,
            vec![NavigationPlace {
                place: FacePlace::Body,
                root_subject: root,
                name: "Structured Sign".to_string(),
                aspects: vec![NavigationAspect {
                    aspect: FaceAspect::Signs,
                    focusable_subjects: subject_ids,
                }],
            }],
            Vec::new(),
        )
        .map_err(StructuredSignFaceRefusal::Navigation)?;
        let memberships = projection_memberships(&face, &inspection);
        let projection = FaceProjection::new(&face, &navigation, memberships)
            .map_err(StructuredSignFaceRefusal::Projection)?;
        Ok(Self {
            inspection,
            face,
            navigation,
            projection,
        })
    }
}

fn add_node_properties(
    properties: &mut Vec<FaceProperty>,
    subject: &str,
    node: &conduit_core::StructuredInfoInspectionNode,
) {
    match &node.member {
        StructuredInfoInspectionMember::Root => {}
        StructuredInfoInspectionMember::RecordField(name) => properties.push(property(
            subject,
            "record-field",
            FacePropertyValue::Identity(name.clone()),
        )),
        StructuredInfoInspectionMember::CollectionItem(index) => properties.push(property(
            subject,
            "collection-index",
            FacePropertyValue::Count(u64::from(*index)),
        )),
        StructuredInfoInspectionMember::VariantPayload => properties.push(property(
            subject,
            "variant-payload",
            FacePropertyValue::Flag(true),
        )),
    }
    add_shape_properties(properties, subject, &node.shape);
}

fn add_shape_properties(
    properties: &mut Vec<FaceProperty>,
    subject: &str,
    shape: &StructuredInfoInspectionShape,
) {
    match shape {
        StructuredInfoInspectionShape::Nominal {
            schema,
            representation,
        } => {
            properties.push(property(
                subject,
                "nominal-schema",
                FacePropertyValue::Identity(schema.as_str().to_string()),
            ));
            add_shape_properties(properties, subject, representation);
        }
        StructuredInfoInspectionShape::Leaf {
            kind,
            byte_len,
            semantic,
        } => {
            properties.push(property(
                subject,
                "leaf-kind",
                FacePropertyValue::Identity(kind.as_str().to_string()),
            ));
            properties.push(property(
                subject,
                "leaf-byte-count",
                FacePropertyValue::Count(u64::from(*byte_len)),
            ));
            if let Some(StructuredInfoLeafSemantic::Quantity(quantity)) = semantic.as_deref() {
                properties.push(property(
                    subject,
                    "quantity-unit",
                    FacePropertyValue::Identity(quantity.unit().semantic_id().to_string()),
                ));
                properties.push(property(
                    subject,
                    "quantity-unit-symbol",
                    FacePropertyValue::Text(quantity.unit().canonical_symbol()),
                ));
                properties.push(property(
                    subject,
                    "quantity-value",
                    FacePropertyValue::Text(alloc::format!(
                        "{}e{}",
                        quantity.coefficient(),
                        quantity.exponent()
                    )),
                ));
            }
        }
        StructuredInfoInspectionShape::Collection { length } => properties.push(property(
            subject,
            "collection-item-count",
            FacePropertyValue::Count(u64::from(*length)),
        )),
        StructuredInfoInspectionShape::Sequence {
            length,
            minimum_items,
            maximum_items,
        } => {
            properties.push(property(
                subject,
                "sequence-item-count",
                FacePropertyValue::Count(u64::from(*length)),
            ));
            properties.push(property(
                subject,
                "sequence-minimum-items",
                FacePropertyValue::Count(u64::from(*minimum_items)),
            ));
            properties.push(property(
                subject,
                "sequence-maximum-items",
                FacePropertyValue::Count(u64::from(*maximum_items)),
            ));
        }
        StructuredInfoInspectionShape::Record {
            schema,
            field_count,
        } => {
            properties.push(property(
                subject,
                "record-schema",
                FacePropertyValue::Identity(schema.as_str().to_string()),
            ));
            properties.push(property(
                subject,
                "record-field-count",
                FacePropertyValue::Count(u64::from(*field_count)),
            ));
        }
        StructuredInfoInspectionShape::Variant { schema, active_tag } => {
            properties.push(property(
                subject,
                "variant-schema",
                FacePropertyValue::Identity(schema.as_str().to_string()),
            ));
            properties.push(property(
                subject,
                "active-variant-tag",
                FacePropertyValue::Identity(active_tag.clone()),
            ));
        }
    }
}

fn projection_memberships(
    face: &Face,
    inspection: &StructuredInfoInspection,
) -> Vec<ProjectionMembership> {
    let mut memberships = Vec::new();
    for node in &inspection.nodes {
        let subject = node_identity(inspection, node.ordinal);
        let depth = node_depth(inspection, node.ordinal);
        memberships.push(membership(ProjectionItem::Subject(subject.clone()), depth));
        for (index, relationship) in face.relationships.iter().enumerate() {
            if relationship.target == subject {
                memberships.push(membership(
                    ProjectionItem::Relationship(index as u16),
                    depth,
                ));
            }
        }
        for (index, property) in face.properties.iter().enumerate() {
            if property.subject == subject {
                memberships.push(membership(ProjectionItem::Property(index as u16), depth));
            }
        }
    }
    memberships
}

fn membership(item: ProjectionItem, depth: FaceDepth) -> ProjectionMembership {
    ProjectionMembership {
        place: FacePlace::Body,
        aspect: FaceAspect::Signs,
        item,
        depth,
    }
}

fn node_depth(inspection: &StructuredInfoInspection, mut ordinal: u16) -> FaceDepth {
    let mut depth = 0u8;
    while let Some(parent) = inspection.nodes[usize::from(ordinal)].parent {
        depth = depth.saturating_add(1);
        ordinal = parent;
    }
    match depth {
        0 => FaceDepth::Primary,
        1 => FaceDepth::Context,
        2 => FaceDepth::Detail,
        _ => FaceDepth::Exact,
    }
}

fn node_identity(inspection: &StructuredInfoInspection, ordinal: u16) -> String {
    format!(
        "structured-sign/{}/{ordinal}",
        hex(&inspection.value_digest)
    )
}

fn node_label(node: &conduit_core::StructuredInfoInspectionNode) -> String {
    match &node.member {
        StructuredInfoInspectionMember::Root => "value".to_string(),
        StructuredInfoInspectionMember::RecordField(name) => format!("field {name}"),
        StructuredInfoInspectionMember::CollectionItem(index) => format!("item {index}"),
        StructuredInfoInspectionMember::VariantPayload => match &node.shape {
            StructuredInfoInspectionShape::Variant { active_tag, .. } => {
                format!("variant {active_tag}")
            }
            _ => "variant payload".to_string(),
        },
    }
}

fn property(subject: &str, name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: subject.to_string(),
        name: name.to_string(),
        value,
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
