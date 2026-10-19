//! Finite artifact identity for the built-in owner Face DOM Back.
//! These bytes grant no source membership or interaction authority.
use conduit_core::{ArtifactId, CapabilityId, CapabilityOffer, ImplementationId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const OWNER_COMPONENT_IDENTITY: &str = "conduit.browser/owner-face-component@1";
const TAG: &str = "conduit-owner-face";
const IMPLEMENTATION: &str = "implementation/browser-owner-face-component@1";
const CAPABILITY: &str = "capability/browser-owner-face-component@1";
const MAXIMUM_BUNDLE_BYTES: usize = 65_536;
const SOURCES: [(&str, &str, usize, &[u8]); 3] = [
    (
        "component",
        "owner-face-component.mjs",
        8192,
        include_bytes!("../../handbook/owner-face-component.mjs"),
    ),
    (
        "presentation",
        "owner-face-presentation.mjs",
        49_152,
        include_bytes!("../../handbook/owner-face-presentation.mjs"),
    ),
    (
        "style",
        "owner-face-component-look.css",
        16_384,
        include_bytes!("../../handbook/owner-face-look.css"),
    ),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerComponentAdmission {
    pub identity: String,
    pub version: u16,
    pub tag: String,
    pub bundle_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OwnerComponentResource {
    pub role: &'static str,
    pub path: &'static str,
    pub maximum_bytes: usize,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OwnerComponentDescriptor {
    pub schema: &'static str,
    pub identity: &'static str,
    pub version: u16,
    pub tag: &'static str,
    pub maximum_bundle_bytes: usize,
    pub resources: Vec<OwnerComponentResource>,
    pub bundle_sha256: String,
    pub artifact_id: String,
    pub implementation_id: &'static str,
    pub capability_id: &'static str,
}

fn bundle_digest() -> String {
    let mut hash = Sha256::new();
    hash.update(b"conduit.browser/owner-face-bundle@1\n");
    for (role, _, _, bytes) in SOURCES {
        hash.update((role.len() as u32).to_be_bytes());
        hash.update(role.as_bytes());
        hash.update((bytes.len() as u32).to_be_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}

pub fn owner_component_descriptor() -> Result<OwnerComponentDescriptor, String> {
    if SOURCES
        .iter()
        .any(|(_, _, maximum, bytes)| bytes.is_empty() || bytes.len() > *maximum)
        || SOURCES
            .iter()
            .map(|(_, _, _, bytes)| bytes.len())
            .sum::<usize>()
            > MAXIMUM_BUNDLE_BYTES
    {
        return Err("owner Face component source graph exceeds its finite byte admission".into());
    }
    let bundle_sha256 = bundle_digest();
    Ok(OwnerComponentDescriptor {
        schema: "conduit.browser/owner-face-component-back@1",
        identity: OWNER_COMPONENT_IDENTITY,
        version: 1,
        tag: TAG,
        maximum_bundle_bytes: MAXIMUM_BUNDLE_BYTES,
        resources: SOURCES
            .iter()
            .map(
                |(role, path, maximum_bytes, bytes)| OwnerComponentResource {
                    role,
                    path,
                    maximum_bytes: *maximum_bytes,
                    bytes: bytes.len(),
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                },
            )
            .collect(),
        artifact_id: format!("artifact/browser-owner-face-component@1/sha256/{bundle_sha256}"),
        bundle_sha256,
        implementation_id: IMPLEMENTATION,
        capability_id: CAPABILITY,
    })
}

pub fn admits_owner_component(admission: &OwnerComponentAdmission) -> bool {
    owner_component_descriptor().is_ok_and(|descriptor| {
        admission.identity == descriptor.identity
            && admission.version == descriptor.version
            && admission.tag == descriptor.tag
            && admission.bundle_sha256 == descriptor.bundle_sha256
    })
}

/// The same configured component Fore with exact reviewed realization bytes.
/// Callers must share this offer in planning and placement revalidation. Missing
/// or mismatched DOM resources require an ordinary generic Plan before Show.
pub fn owner_component_offer() -> Result<CapabilityOffer, String> {
    let descriptor = owner_component_descriptor()?;
    let mut offer = crate::configured_offer();
    offer.capability_id = CapabilityId::from(CAPABILITY);
    offer.implementation.implementation_id = ImplementationId::from(IMPLEMENTATION);
    offer.implementation.artifact_id = ArtifactId::from(descriptor.artifact_id);
    Ok(offer)
}

#[cfg(test)]
mod tests;
