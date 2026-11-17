//! Requested Rust backing for the closed ambient projection. These bounds do
//! not measure RSS, allocator overhead or stacks, and grant no IMAGE ownership.
use crate::PreparedAmbientFaces;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as Shape, StructuredInfoValue,
};
use conduit_face::*;
use conduit_plot::{PortableExpressionNode as Node, PortableExpressionOperation as Op};
use core::mem::size_of;

pub(crate) const IDENTITY_BYTES: usize = 96;
const ID: usize = IDENTITY_BYTES; // ambient/<64 hex>/<clock-weather/weather or composition/N>
const PROPERTY_NAME: usize = 32;

/// Fixed output vectors plus their finite payloads. Builder reserves exactly
/// these capacities:8subjects/relationships/text/disclosures,2composition,
///32properties. Image/quantity/civil typed values are each <=1024B. The input's
///8192B envelope covers all metadata; two independent copies fund property
///payloads and formatted wording, each with a further2x capacity allowance.
pub fn ambient_home_fragment_backing_bound() -> usize {
    size_of::<FaceFragment>()
        + 3 * MAX_FACE_ID_BYTES
        + 8 * (size_of::<FaceSubject>() + ID + 160 + 32)
        + 8 * (size_of::<FaceRelationship>() + 2 * ID)
        + 2 * (size_of::<FaceCompositionRelation>() + 3 * ID)
        + 32 * (size_of::<FaceProperty>() + ID + PROPERTY_NAME)
        + 8 * (size_of::<FaceText>() + ID)
        + 8 * (size_of::<FaceDisclosure>() + ID)
        + 4 * crate::AMBIENT_HOME_MAXIMUM_BYTES as usize
        + 3 * (MAX_FACE_VALUE_BYTES as usize + MAX_FACE_ID_BYTES)
}
fn vec_bytes<T>(value: &alloc::vec::Vec<T>) -> Option<usize> {
    value.capacity().checked_mul(size_of::<T>())
}
/// Actual capacity visitor for this closed fragment family. Unsupported
/// actions/temporal arrays or semantic relationship payloads refuse, rather
/// than silently omitting newly introduced owners.
pub fn ambient_fragment_backing_bytes(f: &FaceFragment) -> Option<usize> {
    if !f.actions.is_empty()
        || f.actions.capacity() != 0
        || !f.temporal_references.is_empty()
        || f.temporal_references.capacity() != 0
        || !f.temporal_facts.is_empty()
        || f.temporal_facts.capacity() != 0
        || f.basis.required_interaction_context.is_some()
    {
        return None;
    }
    let mut n = size_of::<FaceFragment>()
        .checked_add(f.basis.checked_plot_id.0.capacity())?
        .checked_add(f.basis.plan_id.0.capacity())?
        .checked_add(f.basis.active_play_id.0.capacity())?;
    for size in [
        vec_bytes(&f.subjects)?,
        vec_bytes(&f.relationships)?,
        vec_bytes(&f.composition)?,
        vec_bytes(&f.properties)?,
        vec_bytes(&f.text)?,
        vec_bytes(&f.disclosures)?,
    ] {
        n = n.checked_add(size)?;
    }
    for s in &f.subjects {
        n = n
            .checked_add(s.identity.capacity())?
            .checked_add(s.name.capacity())?;
        if let FaceRole::Semantic(kind) = &s.role {
            n = n.checked_add(kind.0.capacity())?;
        }
    }
    for r in &f.relationships {
        if matches!(r.kind, FaceRelationshipKind::Semantic(_)) {
            return None;
        }
        n = n
            .checked_add(r.source.capacity())?
            .checked_add(r.target.capacity())?;
    }
    for c in &f.composition {
        n = n
            .checked_add(c.identity.capacity())?
            .checked_add(c.source.capacity())?
            .checked_add(c.target.capacity())?;
    }
    for p in &f.properties {
        n = n.checked_add(p.retained_backing_bytes()?)?;
    }
    for t in &f.text {
        n = n
            .checked_add(t.subject.capacity())?
            .checked_add(t.text.capacity())?;
    }
    for d in &f.disclosures {
        n = n.checked_add(d.subject.capacity())?;
    }
    Some(n)
}
impl PreparedAmbientFaces {
    /// The exact-schema validator precedes decoding. Thus allocating Type and
    ///value nodes are bounded by this prepared schema, not the global parser
    ///ceiling. Each invariant can retain at most one result per expression node;
    ///invariants run sequentially. Four fragment envelopes cover the two child
    ///builders, composition result and validation/encoding temporaries.
    pub fn projection_workspace_bytes(&self) -> Option<usize> {
        let nodes = value_nodes(&self.home.value_type)?;
        let decoded = self
            .home
            .value_type
            .retained_backing_bytes()?
            .checked_mul(2)?
            .checked_add(nodes.checked_mul(
                2 * (size_of::<StructuredInfoValue>() + size_of::<StructuredFieldValue>()),
            )?)?
            .checked_add(2 * crate::AMBIENT_HOME_MAXIMUM_BYTES as usize)?;
        let factor = [
            &self.wallpaper,
            &self.clock_weather,
            &self.home,
            &self.civil,
            &self.wallpaper_observation,
        ]
        .into_iter()
        .flat_map(|ty| &ty.invariants)
        .try_fold(1usize, |n, p| Some(n.max(expression_nodes(&p.root)?)))?;
        decoded
            .checked_mul(factor.checked_add(2)?)?
            .checked_add(4 * ambient_home_fragment_backing_bound())
    }
}
fn value_nodes(ty: &StructuredInfoType) -> Option<usize> {
    let children = match ty.shape() {
        Shape::Leaf(_) => 0,
        Shape::Nominal { representation, .. } => value_nodes(representation)?,
        Shape::Collection { element, length } => {
            value_nodes(element)?.checked_mul(usize::from(length))?
        }
        Shape::Sequence {
            element,
            maximum_items,
            ..
        } => value_nodes(element)?.checked_mul(usize::from(maximum_items))?,
        Shape::Record { fields, .. } => fields
            .iter()
            .try_fold(0usize, |n, f| n.checked_add(value_nodes(f.value_type())?))?,
        Shape::Variant { cases, .. } => cases
            .iter()
            .try_fold(0usize, |n, c| Some(n.max(value_nodes(c.payload_type())?)))?,
    };
    children.checked_add(1)
}
fn expression_nodes(node: &Node) -> Option<usize> {
    let children = match &node.operation {
        Op::Input | Op::Literal(_) | Op::CanonicalLiteral(_) => 0,
        Op::Constant(_) => return None,
        Op::ValueContract { value, .. } | Op::Projection { value, .. } => expression_nodes(value)?,
        Op::Unary { operand, .. } => expression_nodes(operand)?,
        Op::Binary { left, right, .. } => {
            expression_nodes(left)?.checked_add(expression_nodes(right)?)?
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => expression_nodes(condition)?
            .checked_add(expression_nodes(when_true)?)?
            .checked_add(expression_nodes(when_false)?)?,
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => values
            .iter()
            .try_fold(0usize, |n, v| n.checked_add(expression_nodes(v)?))?,
        Op::Record(fields) => fields
            .iter()
            .try_fold(0usize, |n, (_, v)| n.checked_add(expression_nodes(v)?))?,
        Op::Variant { payload, .. } => expression_nodes(payload)?,
    };
    children.checked_add(1)
}

pub(crate) fn subject_identity(suffix: &str, role: &str) -> alloc::string::String {
    let mut value = alloc::string::String::with_capacity(IDENTITY_BYTES);
    value.push_str("ambient/");
    value.push_str(suffix);
    value.push('/');
    value.push_str(role);
    value
}

/// Minimum independently advertised preparation/projection workspace. Escaped
///contributions and actual route scope have separate lifetime charges.
pub fn ambient_home_workspace_bytes() -> Result<u32, alloc::string::String> {
    let faces = PreparedAmbientFaces::prepare().map_err(|_| "Home model preparation")?;
    faces
        .retained_backing_bytes()
        .and_then(|n| n.checked_add(faces.projection_workspace_bytes()?))
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| "Home workspace overflow".into())
}

pub(crate) fn one<T>(capacity: usize, value: T) -> alloc::vec::Vec<T> {
    let mut values = alloc::vec::Vec::with_capacity(capacity);
    values.push(value);
    values
}
