//! Fixed image::prepare schema factory, not decoded media or content custody.
//! Source is the generated immutable canonical Type, never incoming value bytes.
use super::*;
use conduit_core::{KindId, StructuredFieldType, StructuredVariantCase};

fn add(a: usize, b: usize) -> Result<usize, FaceSceneError> {
    a.checked_add(b).ok_or(FaceSceneError::DocumentBound)
}
fn mul(a: usize, b: usize) -> Result<usize, FaceSceneError> {
    a.checked_mul(b).ok_or(FaceSceneError::DocumentBound)
}
struct Schema<'a> {
    remaining: &'a [u8],
    owned: usize,
}
impl Schema<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], FaceSceneError> {
        let (head, tail) = self
            .remaining
            .split_at_checked(count)
            .ok_or(FaceSceneError::UnsupportedStructure)?;
        self.remaining = tail;
        Ok(head)
    }
    fn number(&mut self) -> Result<usize, FaceSceneError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| FaceSceneError::UnsupportedStructure)?;
        Ok(u32::from_le_bytes(bytes) as usize)
    }
    fn text(&mut self) -> Result<(), FaceSceneError> {
        let length = self.number()?;
        let _ = self.take(length)?;
        // Core decode_type_node copies each borrowed label with String::from.
        self.owned = add(self.owned, length)?;
        Ok(())
    }
    fn node(&mut self, depth: usize) -> Result<(), FaceSceneError> {
        if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(FaceSceneError::UnsupportedStructure);
        }
        let tag = self.take(1)?[0];
        self.text()?;
        if tag == 0 {
            return Ok(());
        }
        // The actual ImageResource type consists only of leaves, records and
        // variants. A changed generated shape refuses until explicitly modeled.
        let element = match tag {
            2 => size_of::<StructuredFieldType>(),
            3 => size_of::<StructuredVariantCase>(),
            _ => return Err(FaceSceneError::UnsupportedStructure),
        };
        let count = self.number()?;
        // Rust1.98 slice stable sort uses its inline small-sort path at <=20;
        // all actual generated image aggregates have at most five children.
        if count == 0 || count > 20 {
            return Err(FaceSceneError::UnsupportedStructure);
        }
        self.owned = add(self.owned, mul(count, element)?)?;
        // Core StructuredInfoTypeNode's largest variants are (KindId,Vec),
        // plus a discriminant; Arc adds strong+weak atomic usize headers.
        // The generated profile uses these aggregate variants only. This is
        // a layout upper bound, not allocator bookkeeping or arbitrary padding.
        let node = size_of::<(KindId, Vec<StructuredFieldType>, usize)>();
        self.owned = add(self.owned, add(node, mul(2, size_of::<usize>())?)?)?;
        for _ in 0..count {
            self.text()?;
            self.node(depth + 1)?;
        }
        Ok(())
    }
}
pub(super) fn preparation_bytes(face: &Face) -> Result<usize, FaceSceneError> {
    if !cfg!(target_has_atomic = "ptr") {
        // A no-Arc Type clone has a different factory lifetime/ownership model.
        return Err(FaceSceneError::UnsupportedStructure);
    }
    let encoded = conduit_face::IMAGERESOURCE_SEMANTIC_TYPE;
    if encoded.first() != Some(&2) {
        // profile() must clone the aggregate Arc, not an owned leaf String.
        return Err(FaceSceneError::UnsupportedStructure);
    }
    // Reject unmodeled predicates on every structured profile before selecting
    // the image profile. This conservative ownership restriction also covers
    // malformed image bytes that fail the later borrowed decoder. It does not
    // replace image::prepare's independent profile/value validation.
    for property in &face.properties {
        if let conduit_face::FacePropertyValue::TypedValue { contract, .. } = &property.value
            && contract
                .value_kind
                .as_str()
                .starts_with("structured-info/profile-")
            && contract.constraints.iter().any(|constraint| {
                !matches!(
                    constraint,
                    conduit_core::ValueConstraint::ByteLength { .. }
                        | conduit_core::ValueConstraint::CanonicalMembership { .. }
                )
            })
        {
            return Err(FaceSceneError::UnsupportedStructure);
        }
    }
    let mut schema = Schema {
        remaining: encoded,
        owned: 0,
    };
    schema.node(1)?;
    if !schema.remaining.is_empty() {
        return Err(FaceSceneError::UnsupportedStructure);
    }
    // Core canonical_bytes reserves the exact encoded Type length. profile()
    // clones its Arc without new backing, hashes a temporary canonical Vec,
    // then formats a 32-byte digest as64hex and an owned profile identity.
    let hex = mul(size_of::<[u8; 32]>(), 2)?;
    let profile = add(add("structured-info/profile-".len(), hex)?, "@1".len())?;
    // Pinned String/Vec growth: final capacity <=2*complete formatted length;
    // old+new realloc overlap <=3*length. No Type factory cache is introduced.
    let profile_generation = add(hex, mul(profile, 3)?)?;
    let later_canonical = add(encoded.len(), mul(profile, 2)?)?;
    add(
        schema.owned,
        profile_generation.max(later_canonical).max(encoded.len()),
    )
}
