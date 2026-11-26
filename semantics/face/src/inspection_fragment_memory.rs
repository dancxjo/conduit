//! Capacity accounting for inspection-only fragments admitted by selected Backs.
//! This measures requested Rust backing, not RSS or authority. Unsupported
//! action/temporal ownership refuses rather than silently undercharging.
use crate::*;
use core::mem::size_of;

fn vec_bytes<T>(value: &alloc::vec::Vec<T>) -> Option<usize> {
    value.capacity().checked_mul(size_of::<T>())
}
/// Actual capacity visitor for this closed fragment family. Unsupported
/// actions/temporal arrays or semantic relationship/composition payloads refuse, rather
/// than silently omitting newly introduced owners.
pub fn inspection_fragment_backing_bytes(f: &FaceFragment) -> Option<usize> {
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
        if matches!(c.kind, FaceCompositionKind::Semantic(_)) {
            return None;
        }
        n = n
            .checked_add(c.identity.capacity())?
            .checked_add(c.source.capacity())?
            .checked_add(c.target.capacity())?;
    }
    for p in &f.properties {
        n = n.checked_add(property_backing_bytes(p)?)?;
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
fn property_backing_bytes(property: &FaceProperty) -> Option<usize> {
    let bytes = match &property.value {
        FacePropertyValue::Identity(text) | FacePropertyValue::Text(text) => text.capacity(),
        FacePropertyValue::BaseImplementationId(id) => id.0.capacity(),
        FacePropertyValue::Count(_) | FacePropertyValue::Signed(_) | FacePropertyValue::Flag(_) => {
            0
        }
        FacePropertyValue::ValueContract(contract) => contract.retained_backing_bytes()?,
        FacePropertyValue::Content(bytes) => bytes.capacity(),
        FacePropertyValue::TypedValue { contract, bytes } => contract
            .retained_backing_bytes()?
            .checked_add(bytes.capacity())?,
    };
    property
        .subject
        .capacity()
        .checked_add(property.name.capacity())?
        .checked_add(bytes)
}
