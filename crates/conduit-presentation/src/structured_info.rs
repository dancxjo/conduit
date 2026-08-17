//! Redacted Presentation projection of one exact structured value Sign.

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
    NavigationAspect, NavigationPlace, NavigationRefusal, Presentation, PresentationAspect,
    PresentationBasis, PresentationDepth, PresentationError, PresentationNavigation,
    PresentationPlace, PresentationProjection, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    ProjectionItem, ProjectionMembership, ProjectionRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredSignPresentation {
    pub inspection: StructuredInfoInspection,
    pub presentation: Presentation,
    pub navigation: PresentationNavigation,
    pub projection: PresentationProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredSignPresentationRefusal {
    Inspection(StructuredInfoInspectionRefusal),
    Presentation(PresentationError),
    Navigation(NavigationRefusal),
    Projection(ProjectionRefusal),
}

impl StructuredSignPresentation {
    pub fn from_sign(
        revision: u64,
        observation: &Observation,
        expected_type: &StructuredInfoType,
    ) -> Result<Self, StructuredSignPresentationRefusal> {
        let inspection = StructuredInfoInspection::from_sign(observation, expected_type)
            .map_err(StructuredSignPresentationRefusal::Inspection)?;
        let mut subjects = Vec::with_capacity(inspection.nodes.len());
        let mut relationships = Vec::with_capacity(inspection.nodes.len().saturating_sub(1));
        let mut properties = Vec::new();
        let mut subject_ids = Vec::with_capacity(inspection.nodes.len());

        for node in &inspection.nodes {
            let identity = node_identity(&inspection, node.ordinal);
            let label = node_label(node);
            subjects.push(PresentationSubject {
                identity: identity.clone(),
                role: if node.parent.is_none() {
                    PresentationRole::Sign
                } else {
                    PresentationRole::Info
                },
                accessibility_name: format!("Structured Info {label}"),
                label,
            });
            subject_ids.push(identity.clone());
            if let Some(parent) = node.parent {
                relationships.push(PresentationRelationship {
                    source: subject_ids[usize::from(parent)].clone(),
                    target: identity.clone(),
                    kind: PresentationRelationshipKind::Contains,
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
                PresentationPropertyValue::Identity(inspection.value_kind.as_str().to_string()),
            ),
            property(
                &root,
                "type-digest",
                PresentationPropertyValue::Identity(hex(&inspection.type_digest)),
            ),
            property(
                &root,
                "value-digest",
                PresentationPropertyValue::Identity(hex(&inspection.value_digest)),
            ),
            property(
                &root,
                "omitted-node-count",
                PresentationPropertyValue::Count(u64::from(inspection.omitted_nodes)),
            ),
            property(
                &root,
                "leaf-content-redacted",
                PresentationPropertyValue::Flag(true),
            ),
        ]);

        let presentation = Presentation::new(
            revision,
            PresentationBasis {
                seed_id: None,
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![inspection.sign_id.clone()],
            },
            subjects,
            relationships,
            properties,
            Vec::new(),
        )
        .map_err(StructuredSignPresentationRefusal::Presentation)?;
        let navigation = PresentationNavigation::new(
            &presentation,
            vec![NavigationPlace {
                place: PresentationPlace::Body,
                root_subject: root,
                label: "Structured Sign".to_string(),
                aspects: vec![NavigationAspect {
                    aspect: PresentationAspect::Signs,
                    focusable_subjects: subject_ids,
                }],
            }],
            Vec::new(),
        )
        .map_err(StructuredSignPresentationRefusal::Navigation)?;
        let memberships = projection_memberships(&presentation, &inspection);
        let projection = PresentationProjection::new(&presentation, &navigation, memberships)
            .map_err(StructuredSignPresentationRefusal::Projection)?;
        Ok(Self {
            inspection,
            presentation,
            navigation,
            projection,
        })
    }
}

fn add_node_properties(
    properties: &mut Vec<PresentationProperty>,
    subject: &str,
    node: &conduit_core::StructuredInfoInspectionNode,
) {
    match &node.member {
        StructuredInfoInspectionMember::Root => {}
        StructuredInfoInspectionMember::RecordField(name) => properties.push(property(
            subject,
            "record-field",
            PresentationPropertyValue::Identity(name.clone()),
        )),
        StructuredInfoInspectionMember::CollectionItem(index) => properties.push(property(
            subject,
            "collection-index",
            PresentationPropertyValue::Count(u64::from(*index)),
        )),
        StructuredInfoInspectionMember::VariantPayload => properties.push(property(
            subject,
            "variant-payload",
            PresentationPropertyValue::Flag(true),
        )),
    }
    match &node.shape {
        StructuredInfoInspectionShape::Leaf {
            kind,
            byte_len,
            semantic,
        } => {
            properties.push(property(
                subject,
                "leaf-kind",
                PresentationPropertyValue::Identity(kind.as_str().to_string()),
            ));
            properties.push(property(
                subject,
                "leaf-byte-count",
                PresentationPropertyValue::Count(u64::from(*byte_len)),
            ));
            if let Some(StructuredInfoLeafSemantic::Quantity(quantity)) = semantic {
                properties.push(property(
                    subject,
                    "quantity-unit",
                    PresentationPropertyValue::Identity(quantity.unit().semantic_id().to_string()),
                ));
                properties.push(property(
                    subject,
                    "quantity-value",
                    PresentationPropertyValue::Signed(quantity.value()),
                ));
            }
        }
        StructuredInfoInspectionShape::Collection { length } => properties.push(property(
            subject,
            "collection-item-count",
            PresentationPropertyValue::Count(u64::from(*length)),
        )),
        StructuredInfoInspectionShape::Record {
            schema,
            field_count,
        } => {
            properties.push(property(
                subject,
                "record-schema",
                PresentationPropertyValue::Identity(schema.as_str().to_string()),
            ));
            properties.push(property(
                subject,
                "record-field-count",
                PresentationPropertyValue::Count(u64::from(*field_count)),
            ));
        }
        StructuredInfoInspectionShape::Variant { schema, active_tag } => {
            properties.push(property(
                subject,
                "variant-schema",
                PresentationPropertyValue::Identity(schema.as_str().to_string()),
            ));
            properties.push(property(
                subject,
                "active-variant-tag",
                PresentationPropertyValue::Identity(active_tag.clone()),
            ));
        }
    }
}

fn projection_memberships(
    presentation: &Presentation,
    inspection: &StructuredInfoInspection,
) -> Vec<ProjectionMembership> {
    let mut memberships = Vec::new();
    for node in &inspection.nodes {
        let subject = node_identity(inspection, node.ordinal);
        let depth = node_depth(inspection, node.ordinal);
        memberships.push(membership(ProjectionItem::Subject(subject.clone()), depth));
        for (index, relationship) in presentation.relationships.iter().enumerate() {
            if relationship.target == subject {
                memberships.push(membership(
                    ProjectionItem::Relationship(index as u16),
                    depth,
                ));
            }
        }
        for (index, property) in presentation.properties.iter().enumerate() {
            if property.subject == subject {
                memberships.push(membership(ProjectionItem::Property(index as u16), depth));
            }
        }
    }
    memberships
}

fn membership(item: ProjectionItem, depth: PresentationDepth) -> ProjectionMembership {
    ProjectionMembership {
        place: PresentationPlace::Body,
        aspect: PresentationAspect::Signs,
        item,
        depth,
    }
}

fn node_depth(inspection: &StructuredInfoInspection, mut ordinal: u16) -> PresentationDepth {
    let mut depth = 0u8;
    while let Some(parent) = inspection.nodes[usize::from(ordinal)].parent {
        depth = depth.saturating_add(1);
        ordinal = parent;
    }
    match depth {
        0 => PresentationDepth::Primary,
        1 => PresentationDepth::Context,
        2 => PresentationDepth::Detail,
        _ => PresentationDepth::Exact,
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

fn property(subject: &str, name: &str, value: PresentationPropertyValue) -> PresentationProperty {
    PresentationProperty {
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
