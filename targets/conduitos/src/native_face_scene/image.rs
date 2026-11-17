//! Present a checked generic ImageResource without opening or acquiring it.
use super::*;
use conduit_core::{BoundedResourceRef, validate_canonical_structured_value};
use conduit_face::{FacePropertyValue, GraphicsImageKey, ImageResource};

pub(super) fn prepare(face: &Face) -> Result<Option<GraphicsImageKey>, FaceSceneError> {
    let ty = ImageResource::semantic_type().map_err(|_| FaceSceneError::InvalidFace)?;
    let profile = ty.profile().map_err(|_| FaceSceneError::InvalidFace)?;
    let schema = ty
        .canonical_bytes()
        .map_err(|_| FaceSceneError::InvalidFace)?;
    for property in &face.properties {
        let FacePropertyValue::TypedValue { contract, bytes } = &property.value else {
            continue;
        };
        if contract.value_kind != *profile.value_kind() {
            continue;
        }
        contract
            .validate(bytes)
            .map_err(|_| FaceSceneError::InvalidFace)?;
        let value =
            validate_canonical_structured_value(bytes).map_err(|_| FaceSceneError::InvalidFace)?;
        if value.type_bytes() != schema {
            return Err(FaceSceneError::InvalidFace);
        }
        let content = value
            .record_field("content")
            .map_err(|_| FaceSceneError::InvalidFace)?
            .ok_or(FaceSceneError::InvalidFace)?;
        let encoded = content
            .primitive_bytes(conduit_core::RESOURCE_REFERENCE_INFO_ID)
            .map_err(|_| FaceSceneError::InvalidFace)?;
        let resource = BoundedResourceRef::validate_encoded(encoded)
            .map_err(|_| FaceSceneError::InvalidFace)?;
        let key = GraphicsImageKey::new(resource.identity.digest(), resource.version.digest())
            .map_err(|_| FaceSceneError::InvalidFace)?;
        // The first semantic image is the current visual contribution. Its
        // exact generation still requires the renderer's independent lease.
        return Ok(Some(key));
    }
    Ok(None)
}

#[cfg(test)]
#[path = "image/tests.rs"]
mod tests;
