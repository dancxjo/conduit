//! One finite built-in browser component Back; never a plugin/module registry.
use conduit_core::{ArtifactId, CapabilityId, HostAdvertisement, ImplementationId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const IDENTITY: &str = "conduit.browser/portable-face-component@1";
const TAG: &str = "conduit-portable-face";
const IMPLEMENTATION: &str = "implementation/browser-portable-face-component@1";
const CAPABILITY: &str = "capability/browser-portable-face-component@1";
const MAXIMUM_BUNDLE_BYTES: usize = 65536;
const SOURCES: [(&str, &str, &[u8]); 5] = [
    (
        "controls",
        "portable-face-controls.mjs",
        include_bytes!("../../../host/assets/portable-face-controls.mjs"),
    ),
    (
        "model",
        "portable-face-model.mjs",
        include_bytes!("../../../host/assets/portable-face-model.mjs"),
    ),
    (
        "combobox",
        "portable-face-combobox.mjs",
        include_bytes!("../../../host/assets/portable-face-combobox.mjs"),
    ),
    (
        "component",
        "portable-face-component.mjs",
        include_bytes!("../../../host/assets/portable-face-component.mjs"),
    ),
    (
        "style",
        "portable-face-look.css",
        include_bytes!("../../../host/assets/portable-face-look.css"),
    ),
];
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComponentAdmission {
    pub identity: String,
    pub version: u16,
    pub tag: String,
    pub bundle_sha256: String,
}
#[derive(Serialize)]
pub(crate) struct ComponentDescriptor {
    schema: &'static str,
    identity: &'static str,
    version: u16,
    tag: &'static str,
    maximum_nodes: u16,
    maximum_depth: u16,
    maximum_in_flight_events: u16,
    maximum_bundle_bytes: usize,
    resources: Vec<Resource>,
    bundle_sha256: String,
    artifact_id: String,
    implementation_id: &'static str,
    capability_id: &'static str,
}
#[derive(Serialize)]
struct Resource {
    role: &'static str,
    path: &'static str,
    sha256: String,
    bytes: usize,
}
fn bundle_digest() -> String {
    let mut hash = Sha256::new();
    hash.update(b"conduit.browser/portable-face-bundle@1\n");
    for (role, _, bytes) in SOURCES {
        hash.update((role.len() as u32).to_be_bytes());
        hash.update(role.as_bytes());
        hash.update((bytes.len() as u32).to_be_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}
fn artifact(bundle: &str) -> String {
    format!("artifact/browser-portable-face-component@1/sha256/{bundle}")
}
pub(crate) fn descriptor() -> ComponentDescriptor {
    let bundle_sha256 = bundle_digest();
    ComponentDescriptor {
        schema: "conduit.browser/portable-face-component-back@1",
        identity: IDENTITY,
        version: 1,
        tag: TAG,
        maximum_nodes: 2048,
        maximum_depth: 12,
        maximum_in_flight_events: 1,
        maximum_bundle_bytes: MAXIMUM_BUNDLE_BYTES,
        resources: SOURCES
            .iter()
            .map(|(role, path, bytes)| Resource {
                role,
                path,
                sha256: format!("{:x}", Sha256::digest(bytes)),
                bytes: bytes.len(),
            })
            .collect(),
        artifact_id: artifact(&bundle_sha256),
        bundle_sha256,
        implementation_id: IMPLEMENTATION,
        capability_id: CAPABILITY,
    }
}
pub(crate) fn expected_admission() -> ComponentAdmission {
    ComponentAdmission {
        identity: IDENTITY.into(),
        version: 1,
        tag: TAG.into(),
        bundle_sha256: bundle_digest(),
    }
}
fn source_graph_within_bounds() -> bool {
    SOURCES
        .iter()
        .map(|(_, _, bytes)| bytes.len())
        .sum::<usize>()
        <= MAXIMUM_BUNDLE_BYTES
        && SOURCES.iter().all(|(role, _, bytes)| {
            bytes.len()
                <= match *role {
                    "controls" | "style" => 16384,
                    _ => 8192,
                }
        })
}

#[derive(Clone, Copy)]
pub(crate) enum RendererProfile {
    Ordinary,
    PortableComponent,
}
impl RendererProfile {
    pub(crate) fn admit(
        component: bool,
        admission: Option<&ComponentAdmission>,
    ) -> Result<Self, String> {
        match (component, admission) {
            (false, None) => Ok(Self::Ordinary),
            (true, Some(receipt))
                if receipt == &expected_admission() && source_graph_within_bounds() =>
            {
                Ok(Self::PortableComponent)
            }
            _ => Err(
                "browser component identity, version, tag or source digest is not admitted".into(),
            ),
        }
    }
    pub(super) fn host(
        self,
        host_id: conduit_core::HostId,
        boot_id: conduit_core::BootId,
    ) -> HostAdvertisement {
        let mut host = crate::installed_browser::membership_advertisement(host_id, boot_id);
        if matches!(self, Self::PortableComponent) {
            let mut offer = crate::installed_browser::dom_mask::configured_offer();
            let kind = offer.kind_id.clone();
            offer.capability_id = CapabilityId::from(CAPABILITY);
            offer.implementation.implementation_id = ImplementationId::from(IMPLEMENTATION);
            offer.implementation.artifact_id = ArtifactId::from(artifact(&bundle_digest()));
            host.capabilities.retain(|offer| offer.kind_id != kind);
            host.capabilities.push(offer);
        }
        host
    }
}
#[cfg(test)]
mod tests;
