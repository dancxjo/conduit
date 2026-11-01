//! Lossless serialization specialization for a read-only leaf Face profile.
//! Unsupported collections/constraints refuse; no field is silently omitted.
//! Wire bytes remain the canonical serde Face, not another Face representation.
use conduit_face::{Face, FaceProperty, FacePropertyValue};
use serde::{
    ser::{SerializeSeq, SerializeStruct},
    Serialize, Serializer,
};
pub struct Leaf<'a>(&'a Face);
pub fn checked(face: &Face) -> Result<Leaf<'_>, ()> {
    if !face.composition.is_empty()
        || !face.actions.is_empty()
        || !face.disclosures.is_empty()
        || !face.temporal_references.is_empty()
        || !face.temporal_facts.is_empty()
    {
        return Err(());
    }
    for property in &face.properties {
        let contract = match &property.value {
            FacePropertyValue::TypedValue { contract, .. }
            | FacePropertyValue::ValueContract(contract) => Some(contract),
            _ => None,
        };
        if contract.is_some_and(|c| !c.constraints.is_empty()) {
            return Err(());
        }
    }
    Ok(Leaf(face))
}
impl Serialize for Leaf<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let face = self.0;
        let empty: &[()] = &[];
        // JSON carries fields, not serde's incidental Rust struct names. Avoid
        // power-of-two metadata literals: Xtensa pools those in scarce DRAM.
        let mut object = serializer.serialize_struct("leaf-Face", 13)?;
        object.serialize_field("identity", &face.identity)?;
        object.serialize_field("revision", &face.revision)?;
        object.serialize_field("basis", &face.basis)?;
        object.serialize_field("interaction_context", &Context(&face.interaction_context))?;
        object.serialize_field("subjects", &face.subjects)?;
        object.serialize_field("relationships", &Relationships(&face.relationships))?;
        object.serialize_field("composition", empty)?;
        object.serialize_field("properties", &Properties(&face.properties))?;
        object.serialize_field("text", &Texts(&face.text))?;
        object.serialize_field("actions", empty)?;
        object.serialize_field("disclosures", empty)?;
        object.serialize_field("temporal_references", empty)?;
        object.serialize_field("temporal_facts", empty)?;
        object.end()
    }
}
struct Properties<'a>(&'a [FaceProperty]);
impl Serialize for Properties<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for property in self.0 {
            sequence.serialize_element(&Property(property))?;
        }
        sequence.end()
    }
}
struct Property<'a>(&'a FaceProperty);
impl Serialize for Property<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_struct("FaceProperty", 3)?;
        object.serialize_field("subject", &self.0.subject)?;
        object.serialize_field("name", &self.0.name)?;
        object.serialize_field("value", &Value(&self.0.value))?;
        object.end()
    }
}
#[derive(Serialize)]
#[serde(rename = "face-value-contract")]
struct Contract<'a> {
    value_kind: &'a conduit_core::KindId,
    maximum_bytes: u32,
    constraints: &'a [()],
}
struct Context<'a>(&'a conduit_face::FaceInteractionContext);
impl Serialize for Context<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_struct("face-context-reference", 2)?;
        object.serialize_field("identity", &self.0.identity)?;
        object.serialize_field("basis", &ContextBasis(&self.0.basis))?;
        object.end()
    }
}
struct ContextBasis<'a>(&'a [conduit_face::FaceContextBasis]);
impl Serialize for ContextBasis<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "face-context-statement")]
        struct Entry<'a> {
            source: &'a str,
            relationship: &'a conduit_face::FaceRelationshipKind,
            target: &'a str,
        }
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for entry in self.0 {
            sequence.serialize_element(&Entry {
                source: &entry.source,
                relationship: &entry.relationship,
                target: &entry.target,
            })?;
        }
        sequence.end()
    }
}
struct Relationships<'a>(&'a [conduit_face::FaceRelationship]);
impl Serialize for Relationships<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "face-relationship")]
        struct Entry<'a> {
            source: &'a str,
            target: &'a str,
            kind: &'a conduit_face::FaceRelationshipKind,
        }
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for entry in self.0 {
            sequence.serialize_element(&Entry {
                source: &entry.source,
                target: &entry.target,
                kind: &entry.kind,
            })?;
        }
        sequence.end()
    }
}
struct Texts<'a>(&'a [conduit_face::FaceText]);
impl Serialize for Texts<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "face-text")]
        struct Entry<'a> {
            subject: &'a str,
            text: &'a str,
        }
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for entry in self.0 {
            sequence.serialize_element(&Entry {
                subject: &entry.subject,
                text: &entry.text,
            })?;
        }
        sequence.end()
    }
}
impl<'a> From<&'a conduit_core::CheckedValueContract> for Contract<'a> {
    fn from(c: &'a conduit_core::CheckedValueContract) -> Self {
        Self {
            value_kind: &c.value_kind,
            maximum_bytes: c.maximum_bytes,
            constraints: &[],
        }
    }
}
struct Value<'a>(&'a FacePropertyValue);
impl Serialize for Value<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        enum Wire<'a> {
            Identity(&'a str),
            BaseImplementationId(&'a conduit_core::BaseImplementationId),
            Text(&'a str),
            Count(u64),
            Signed(i64),
            Flag(bool),
            ValueContract(Contract<'a>),
            Content(&'a [u8]),
            TypedValue {
                contract: Contract<'a>,
                bytes: &'a [u8],
            },
        }
        match self.0 {
            FacePropertyValue::Identity(v) => Wire::Identity(v),
            FacePropertyValue::BaseImplementationId(v) => Wire::BaseImplementationId(v),
            FacePropertyValue::Text(v) => Wire::Text(v),
            FacePropertyValue::Count(v) => Wire::Count(*v),
            FacePropertyValue::Signed(v) => Wire::Signed(*v),
            FacePropertyValue::Flag(v) => Wire::Flag(*v),
            FacePropertyValue::ValueContract(v) => Wire::ValueContract(v.into()),
            FacePropertyValue::Content(v) => Wire::Content(v),
            FacePropertyValue::TypedValue { contract, bytes } => Wire::TypedValue {
                contract: contract.into(),
                bytes,
            },
        }
        .serialize(serializer)
    }
}
