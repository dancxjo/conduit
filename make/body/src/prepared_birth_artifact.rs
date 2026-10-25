//! Prepared birth production and exact ISO byte binding through admitted handles.
//! A successful receipt proves inert bytes and readback, never boot or join.
use crate::PreparedDeployment;
use conduit_body::{PreparedBirthMedia, PreparedBirthMediaBinding};
use conduit_host_make::{canonical_profile_json, verify_image_binding, HostImage};
use sha2::{Digest, Sha256};

mod iso;
mod manifest;
pub use iso::bind_prepared_birth_iso;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedBirthArtifactRefusal {
    InvalidSelection,
    InvalidProfile,
    InvalidManifest,
    UnsupportedConsumer,
    SlotMissing,
    SlotOccupied,
    SlotAmbiguous,
    InvalidIso,
    BoundExhausted,
    Cancelled,
    Io,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedBirthArtifactReceipt {
    pub schema: String,
    pub body_id: String,
    pub host_entry: String,
    pub first_host_entry: String,
    pub construction_id: String,
    pub configuration_id: String,
    pub profile_id: String,
    pub build_id: String,
    pub target: String,
    pub base_image_id: String,
    pub base_sha256: String,
    pub frame_sha256: String,
    pub artifact_sha256: String,
    pub artifact_bytes: u64,
    pub frame_offset: u64,
    pub boot_observed: bool,
    pub join_observed: bool,
}

impl PreparedDeployment {
    pub fn birth_media_for_image(
        &self,
        host_entry: &str,
        image: &HostImage,
    ) -> Result<PreparedBirthMedia, PreparedBirthArtifactRefusal> {
        use PreparedBirthArtifactRefusal as E;
        if !self.selected_hosts().iter().any(|name| name == host_entry) {
            return Err(E::InvalidSelection);
        }
        let host = self
            .construction()
            .hosts()
            .iter()
            .find(|host| host.description.name == host_entry)
            .ok_or(E::InvalidSelection)?;
        let profile = host.configuration.profile();
        let canonical = canonical_profile_json(profile).map_err(|_| E::InvalidProfile)?;
        let profile_id = format!("sha256:{:x}", Sha256::digest(&canonical));
        if image.manifest.profile_id != profile_id
            || image.manifest.source_configuration_id.as_deref()
                != Some(host.configuration.configuration_id())
            || image.manifest.output != host.description.spore.output
            || image.manifest.target != profile.target.key()
        {
            return Err(E::InvalidProfile);
        }
        manifest::verify_image_closure(image)?;
        // Compare every PROFILE-owned payload field, rather than trusting a
        // profile-id string attached to unrelated machinery.
        let p = &image.payload;
        let sorted = |mut values: Vec<String>| {
            values.sort();
            values
        };
        let mut resources = profile.resources.clone();
        resources.sort_by(|a, b| a.id.cmp(&b.id));
        let mut bases = profile.bases.clone();
        bases.sort_by(|a, b| a.id.cmp(&b.id));
        let mut drivers = profile.drivers.clone();
        drivers.sort_by(|a, b| a.id.cmp(&b.id));
        if p.profile_fragments != sorted(profile.fragments.clone())
            || p.make_descriptor != profile.target.make_descriptor
            || p.implementations
                != sorted(
                    profile
                        .capabilities
                        .iter()
                        .map(|c| c.implementation.clone())
                        .collect(),
                )
            || p.host_calls != sorted(profile.host_calls.clone())
            || p.resources != sorted(profile.resources.iter().map(|r| r.id.clone()).collect())
            || p.resource_budgets != resources
            || p.base_selections != bases
            || p.driver_selections != drivers
            || p.bases != sorted(profile.bases.iter().map(|b| b.id.clone()).collect())
            || p.drivers != sorted(profile.drivers.iter().map(|d| d.id.clone()).collect())
            || p.lines != sorted(profile.lines.clone())
            || p.presenters
                != sorted(
                    profile
                        .presenters
                        .iter()
                        .map(|v| v.implementation.clone())
                        .collect(),
                )
            || p.facilities != sorted(profile.facilities.clone())
            || p.bounds != profile.bounds
        {
            return Err(E::InvalidProfile);
        }
        PreparedBirthMedia::from_evidence(
            self.birth().evidence(),
            PreparedBirthMediaBinding {
                host_entry: host_entry.into(),
                first_host_entry: self.first_host().into(),
                construction_id: self.construction().description_id().into(),
                configuration_id: host.configuration.configuration_id().into(),
                profile_id,
                build_id: image.manifest.build_id.clone(),
                target: image.manifest.target.clone(),
            },
        )
        .map_err(|_| E::InvalidSelection)
    }
}

#[cfg(test)]
mod tests;
