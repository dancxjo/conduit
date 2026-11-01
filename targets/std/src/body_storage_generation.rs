//! Explicit quiescent storage inventory selection. Capacity observations are
//! planning evidence; they issue neither authority nor future disk reservations.
use crate::{body_storage::SelectedBodyStorage, StdHost};
use conduit_core as c;
use std::{
    fs::{File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
    sync::Arc,
};
mod census;
mod preparation;
use preparation::prepare;
#[cfg(test)]
use preparation::snapshot_name;

pub struct BodyStorageSnapshotRequest {
    pub reference: c::BoundedResourceRef,
    pub access: c::ResourceAccessMode,
}
pub struct BodyStorageStateRequest {
    pub binding: crate::state_value::DurableStateBinding,
    pub type_name: String,
    pub value_type: c::StructuredInfoType,
    pub default_value: c::StructuredInfoValue,
}
pub struct BodyStorageGenerationRequest {
    pub snapshot: Option<BodyStorageSnapshotRequest>,
    pub state: Option<BodyStorageStateRequest>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyStorageGenerationEvidence {
    pub offer_generation: c::OfferGeneration,
    pub retained_snapshots: u32,
    pub retained_states: u32,
    pub allocated_bytes: u64,
    pub logical_bytes: u64,
    pub filesystem_available_bytes: u64,
    pub preparation_bytes: u64,
}
pub struct SelectedBodyStorageGeneration {
    host: c::HostId,
    boot: c::BootId,
    generation: c::OfferGeneration,
    reference: Option<c::BoundedResourceRef>,
    snapshot: Option<c::ResourceOffer>,
    state: Option<c::ResourceOffer>,
    base: c::BaseProviderEntry,
    evidence: BodyStorageGenerationEvidence,
    binding: Option<crate::state_value::DurableStateBinding>,
}
impl SelectedBodyStorageGeneration {
    pub fn state_binding(&self) -> Option<&crate::state_value::DurableStateBinding> {
        self.binding.as_ref()
    }
    pub fn snapshot_resource(&self) -> Option<&c::ResourceOffer> {
        self.snapshot.as_ref()
    }
    pub fn state_resource(&self) -> Option<&c::ResourceOffer> {
        self.state.as_ref()
    }
    pub fn snapshot_reference(&self) -> Option<&c::BoundedResourceRef> {
        self.reference.as_ref()
    }
    pub fn offer_generation(&self) -> c::OfferGeneration {
        self.generation
    }
    pub fn evidence(&self) -> &BodyStorageGenerationEvidence {
        &self.evidence
    }
}
impl StdHost {
    pub fn selected_body_storage_generation(&self) -> Option<&SelectedBodyStorageGeneration> {
        let generation = self.selected_body_storage.as_ref()?.generation.as_ref()?;
        (generation.host == self.advertisement.host_id
            && generation.boot == self.advertisement.boot_id
            && generation.generation == self.advertisement.offer_generation)
            .then_some(generation)
    }
    /// Root explicitly selects exact immutable inventory while no child Play
    /// owns resources. Existing Source backing transfers without release/regrant.
    pub fn select_body_storage_generation(
        &mut self,
        request: BodyStorageGenerationRequest,
    ) -> Result<BodyStorageGenerationEvidence, String> {
        let storage = self
            .selected_body_storage
            .as_ref()
            .ok_or("Body storage is not selected")?;
        if Arc::strong_count(storage) != 1 || Arc::weak_count(storage) != 0 {
            return Err("storage generation has an outstanding descriptor holder".into());
        }
        let old = storage.generation.as_ref();
        if old.is_some_and(|prior| {
            prior.host != self.advertisement.host_id
                || prior.boot != self.advertisement.boot_id
                || !prior
                    .base
                    .resources
                    .iter()
                    .all(|resource| self.advertisement.resources.contains(resource))
                || !prior
                    .base
                    .capabilities
                    .iter()
                    .all(|capability| self.advertisement.capabilities.contains(capability))
                || !self.advertisement.bases.iter().any(|base| {
                    base.base_id == prior.base.base_id
                        && base.provider_instance_id == prior.base.provider_instance_id
                        && base.provider_generation == prior.base.provider_generation
                })
        }) {
            return Err("selected storage inventory differs from current Host".into());
        }
        let generation = c::OfferGeneration(
            self.advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or("storage offer generation exhausted")?,
        );
        let staged = prepare(storage, &self.advertisement, generation, request)?;
        let mut staged = staged;
        let new_resources = staged.base.resources.clone();
        let new_capabilities = staged.base.capabilities.clone();
        let new_provider = c::BaseProviderAdvertisement {
            base_id: staged.base.base_id.clone(),
            provider_instance_id: staged.base.provider_instance_id.clone(),
            provider_generation: staged.base.provider_generation,
            implementation_id: staged.base.implementation_id.clone(),
            mechanism_family: staged.base.mechanism_family.clone(),
            enforcement_class: staged.base.enforcement_class,
            lifecycle: staged.base.lifecycle,
            capability_ids: new_capabilities
                .iter()
                .map(|c| c.capability_id.clone())
                .collect(),
            resource_pool_ids: new_resources.iter().map(|r| r.pool_id.clone()).collect(),
        };
        let backing = self
            .native_source
            .as_ref()
            .and_then(|source| source.backing.as_ref());
        let ledger = self.kernel_resources.prepare_storage_refresh(
            &self.advertisement,
            old.map(|g| &g.base),
            &staged.base,
            backing,
        )?;
        let prior_resource_capacity = self.advertisement.resources.capacity();
        let prior_capability_capacity = self.advertisement.capabilities.capacity();
        let prior_base_capacity = self.advertisement.bases.capacity();
        self.advertisement
            .resources
            .try_reserve_exact(
                new_resources
                    .len()
                    .saturating_sub(old.map_or(0, |g| g.base.resources.len())),
            )
            .map_err(|_| "storage advertisement resource backing unavailable")?;
        self.advertisement
            .capabilities
            .try_reserve_exact(
                new_capabilities
                    .len()
                    .saturating_sub(old.map_or(0, |g| g.base.capabilities.len())),
            )
            .map_err(|_| "storage advertisement capability backing unavailable")?;
        self.advertisement
            .bases
            .try_reserve_exact(usize::from(old.is_none()))
            .map_err(|_| "storage advertisement Base backing unavailable")?;
        let growth = vector_growth(
            prior_resource_capacity,
            self.advertisement.resources.capacity(),
            std::mem::size_of::<c::ResourceOffer>(),
        )? + vector_growth(
            prior_capability_capacity,
            self.advertisement.capabilities.capacity(),
            std::mem::size_of::<c::CapabilityOffer>(),
        )? + vector_growth(
            prior_base_capacity,
            self.advertisement.bases.capacity(),
            std::mem::size_of::<c::BaseProviderAdvertisement>(),
        )?;
        let registry_growth = if old.is_none() {
            self.base_registry
                .entries()
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .and_then(|n| n.checked_mul(std::mem::size_of::<c::BaseProviderEntry>()))
                .ok_or("registry vector growth overflow")? as u64
        } else {
            0
        };
        staged.evidence.preparation_bytes = staged
            .evidence
            .preparation_bytes
            .checked_add(ledger.workspace_bytes)
            .and_then(|n| n.checked_add(growth))
            .and_then(|n| n.checked_add(registry_growth))
            .ok_or("storage metadata workspace overflow")?;
        if staged.evidence.preparation_bytes > storage.bounds().preparation_bytes {
            return Err(format!(
                "storage generation preparation requires {} bytes, selected {}",
                staged.evidence.preparation_bytes,
                storage.bounds().preparation_bytes
            ));
        }
        let registry = self
            .base_registry
            .prepare_update(
                old.map(|g| (&g.base.provider_instance_id, g.base.provider_generation)),
                staged.base.clone(),
            )
            .map_err(|e| format!("storage Base selection: {e:?}"))?;
        // The registry guard holds only its field. Transfer the same established
        // spoken provider generations without borrowing/replacing the Host.
        let prior = self.advertisement.offer_generation;
        let boot = &self.advertisement.boot_id;
        if self
            .playback
            .as_ref()
            .is_some_and(|p| p.boot_id != *boot || p.offer_generation != prior)
            || self
                .wav_artifact
                .as_ref()
                .is_some_and(|p| p.boot_id != *boot || p.offer_generation != prior)
        {
            return Err("retained spoken resource differs from current Host offer".into());
        }
        if let Some(adapter) = &mut self.speech_synthesis {
            adapter
                .advance_offer_generation(&self.advertisement.host_id, boot, prior, generation)
                .map_err(|e| format!("retained speech provider: {e:?}"))?;
        }
        if let Some(playback) = &mut self.playback {
            playback.offer_generation = generation;
        }
        if let Some(artifact) = &mut self.wav_artifact {
            artifact.offer_generation = generation;
        }
        // Every check and vector growth is complete. Move only storage-owned
        // entries; unrelated capability payloads and pool objects stay owned.
        if let Some(old) = old {
            self.advertisement.resources.retain(|r| {
                !old.base
                    .resources
                    .iter()
                    .any(|owned| owned.pool_id == r.pool_id)
            });
            self.advertisement.capabilities.retain(|r| {
                !old.base
                    .capabilities
                    .iter()
                    .any(|owned| owned.capability_id == r.capability_id)
            });
            self.advertisement
                .bases
                .retain(|r| r.base_id != old.base.base_id);
        }
        self.advertisement.resources.extend(new_resources);
        self.advertisement.resources.sort_unstable();
        self.advertisement.capabilities.extend(new_capabilities);
        self.advertisement
            .capabilities
            .sort_unstable_by(|a, b| a.capability_id.cmp(&b.capability_id));
        self.advertisement.bases.push(new_provider);
        self.advertisement.offer_generation = generation;
        registry.commit();
        self.kernel_resources.apply_storage_refresh(ledger);
        if let Some(backing) = self
            .native_source
            .as_mut()
            .and_then(|source| source.backing.as_mut())
        {
            backing.adopt_refreshed_ledger(&self.kernel_resources);
        }
        let evidence = staged.evidence.clone();
        Arc::get_mut(self.selected_body_storage.as_mut().expect("selected above"))
            .expect("exclusive strong and weak ownership checked before commit")
            .generation = Some(staged);
        Ok(evidence)
    }
}
fn open_directory(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| format!("storage directory: {e}"))
}

#[cfg(test)]
mod tests;

fn vector_growth(before: usize, after: usize, item: usize) -> Result<u64, String> {
    if after <= before {
        return Ok(0);
    }
    before
        .checked_add(after)
        .and_then(|n| n.checked_mul(item))
        .map(|n| n as u64)
        .ok_or("storage vector growth overflow".into())
}
