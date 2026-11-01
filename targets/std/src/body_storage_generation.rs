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
use census::StorageCensus;

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
        staged.evidence.preparation_bytes = staged
            .evidence
            .preparation_bytes
            .checked_add(ledger.workspace_bytes)
            .and_then(|n| n.checked_add(growth))
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
fn prepare(
    storage: &SelectedBodyStorage,
    host: &c::HostAdvertisement,
    generation: c::OfferGeneration,
    request: BodyStorageGenerationRequest,
) -> Result<SelectedBodyStorageGeneration, String> {
    if host.host_id.as_str().is_empty() || host.host_id.as_str().len() > 128 {
        return Err("snapshot owner exceeds finite record profile".into());
    }
    if request.snapshot.is_none() && request.state.is_none() {
        return Err("storage generation requires a selected operation".into());
    }
    let bounds = storage.bounds();
    let base_id: c::HostBaseId = format!("{}/base", storage.pool_namespace()).into();
    let mut resources = Vec::new();
    let mut capabilities = Vec::new();
    let mut reference = None;
    let mut snapshot = None;
    let mut state = None;
    let mut binding = None;
    let mut expected_state_key = None;
    let mut expected_name = None;
    let mut publishing = false;
    if let Some(selected) = request.snapshot {
        let r = &selected.reference;
        let encoded = r
            .encode()
            .map_err(|e| format!("snapshot reference: {e:?}"))?;
        if encoded.len() > 512
            || r.extent.bytes == 0
            || r.extent.bytes > u64::from(bounds.maximum_record_bytes)
            || r.extent.items != Some(1)
            || r.lifetime.expires_at.is_some()
            || r.access_class.as_str() != "resource/snapshot@1"
            || r.content_profile.as_str() != "text/json-utf8@1"
        {
            return Err("snapshot reference exceeds selected exact implementation".into());
        }
        publishing = selected.access == c::ResourceAccessMode::WriteCandidatePublish;
        let resource = c::ResourceOffer {
            pool_id: format!("{}/snapshot", storage.pool_namespace()).into(),
            class_id: r.access_class.clone(),
            capacity_units: 1,
            compute: None,
            content: Some(c::ResourceContentOffer {
                contract: c::ResourceContentRequirement {
                    identity: r.identity,
                    version: r.lifetime.version,
                    content_profile: r.content_profile.clone(),
                    maximum_bytes: r.extent.bytes as u32,
                    maximum_items: 1,
                    retention: c::ResourceRetention::ExternalDurable,
                    sharing: c::ResourceSharing::SingleWriterPublished,
                    access: selected.access,
                    generation_slots: 1,
                    reader_leases: 1,
                    publication_slots: if publishing { 1 } else { 0 },
                    sensitive: false,
                },
                owner_host: host.host_id.clone(),
                owner_boot: host.boot_id.clone(),
                base_id: base_id.clone(),
                residence_profile: crate::linux_snapshot::PROFILE.into(),
            }),
        };
        capabilities.push(crate::linux_snapshot::offer_for_resource(&resource)?);
        expected_name = Some(snapshot_name(&host.host_id, r));
        reference = Some(selected.reference);
        resources.push(resource.clone());
        snapshot = Some(resource);
    }
    if let Some(selected) = request.state {
        selected
            .binding
            .validate()
            .map_err(|e| format!("selected State binding: {e:?}"))?;
        if selected.binding.maximum_value_bytes > bounds.maximum_state_bytes
            || selected.binding.body.len() > 128
            || selected.binding.state.as_str().len() > 128
            || selected.binding.value_kind.as_str().len() > 128
        {
            return Err("selected State binding exceeds finite configured profile".into());
        }
        expected_state_key = Some(
            crate::state_value::FileDurableStateResidence::storage_key(&selected.binding)
                .map_err(|e| format!("State key: {e:?}"))?,
        );
        if bounds.maximum_state_bytes < conduit_std_offers::STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES {
            return Err(
                "selected State bound cannot cover advertised durable implementation".into(),
            );
        }
        let offer = conduit_std_offers::state_value_durable_std_offer(
            &selected.type_name,
            &selected.value_type,
            &selected.default_value,
        )
        .map_err(|e| format!("State specialization: {e:?}"))?;
        if offer.outputs[0].value_kind != selected.binding.value_kind {
            return Err("State binding differs from selected typed specialization".into());
        }
        binding = Some(selected.binding);
        capabilities.push(offer);
        let resource = c::ResourceOffer {
            pool_id: format!("{}/state", storage.pool_namespace()).into(),
            class_id: conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS.into(),
            capacity_units: 1,
            compute: None,
            content: None,
        };
        resources.push(resource.clone());
        state = Some(resource);
    }
    let census = StorageCensus::inspect(
        storage,
        expected_name.as_deref(),
        publishing,
        expected_state_key.as_deref(),
    )?;
    let base = c::BaseProviderEntry {
        base_id,
        provider_instance_id: format!("{}/directory", storage.pool_namespace()).into(),
        provider_generation: generation.0,
        implementation_id: "linux/body-storage-directory@1".into(),
        mechanism_family: "storage/protected-file".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
        lifecycle: c::BaseLifecycle::Ready,
        capabilities,
        resources,
    };
    // Closed storage factory descriptors only: snapshot/typed State metadata
    // contains finite strings, vectors and canonical-value bytes, no recursive
    // numerical Quantity boxes from unrelated Host offers. JSON scratch stays
    // local to this quote; existing Host/provider payloads are never cloned.
    let descriptor_bound = closed_descriptor_model(&base)?
        .checked_mul(5)
        .ok_or("storage descriptor workspace overflow")?;
    let retained_bound = storage
        .generation
        .as_ref()
        .map(|g| closed_descriptor_model(&g.base))
        .transpose()?
        .unwrap_or(0)
        .checked_mul(2)
        .ok_or("retained storage workspace overflow")?;
    let binding_bytes = binding.as_ref().map_or(0, |binding| {
        binding.body.capacity() + binding.state.as_str().len() + binding.value_kind.as_str().len()
    });
    let preparation = storage
        .custody_preparation_bytes()
        .checked_add(
            (std::mem::size_of::<SelectedBodyStorageGeneration>()
                + 2 * std::mem::size_of::<usize>()
                + descriptor_bound
                + retained_bound
                + binding_bytes) as u64,
        )
        .and_then(|v| v.checked_add(census.workspace_bytes))
        .and_then(|v| {
            v.checked_add(if snapshot.is_some() {
                conduit_linux_snapshot_store::preparation_byte_bound(&host.host_id)
            } else {
                0
            })
        })
        .and_then(|v| {
            v.checked_add(if state.is_some() {
                crate::state_value::durable_preparation_byte_bound(bounds.maximum_state_bytes)
            } else {
                0
            })
        })
        .ok_or("storage preparation overflow")?;
    if preparation > storage.bounds().preparation_bytes {
        return Err(format!(
            "storage generation preparation requires {preparation} bytes, selected {}",
            storage.bounds().preparation_bytes
        ));
    }
    let evidence = BodyStorageGenerationEvidence {
        offer_generation: generation,
        retained_snapshots: census.snapshots,
        retained_states: census.states,
        allocated_bytes: census.allocated,
        logical_bytes: census.logical,
        filesystem_available_bytes: census.available,
        preparation_bytes: preparation,
    };
    Ok(SelectedBodyStorageGeneration {
        host: host.host_id.clone(),
        boot: host.boot_id.clone(),
        generation,
        reference,
        snapshot,
        state,
        base,
        evidence,
        binding,
    })
}
fn snapshot_name(host: &c::HostId, reference: &c::BoundedResourceRef) -> String {
    conduit_linux_snapshot_store::snapshot_storage_key(host, reference)
        .trim_start_matches("resource/")
        .into()
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
fn closed_descriptor_model(base: &c::BaseProviderEntry) -> Result<usize, String> {
    let wire = serde_json::to_vec(base).map_err(|e| e.to_string())?;
    let tree: serde_json::Value = serde_json::from_slice(&wire).map_err(|e| e.to_string())?;
    json_backing(&tree)?
        .checked_add(wire.capacity())
        .and_then(|n| n.checked_add(std::mem::size_of::<c::BaseProviderEntry>()))
        .ok_or("closed storage descriptor workspace overflow".into())
}
fn json_backing(value: &serde_json::Value) -> Result<usize, String> {
    let own = std::mem::size_of::<serde_json::Value>();
    let extra = match value {
        serde_json::Value::String(s) => s.capacity(),
        serde_json::Value::Array(items) => items.iter().try_fold(
            items.capacity() * std::mem::size_of::<serde_json::Value>(),
            |n, v| {
                n.checked_add(json_backing(v)?)
                    .ok_or_else(|| "descriptor array footprint overflow".to_string())
            },
        )?,
        serde_json::Value::Object(items) => items.iter().try_fold(
            items.len() * std::mem::size_of::<(String, serde_json::Value)>(),
            |n, (k, v)| {
                n.checked_add(k.capacity())
                    .and_then(|n| n.checked_add(json_backing(v).ok()?))
                    .ok_or_else(|| "descriptor object footprint overflow".to_string())
            },
        )?,
        _ => 0,
    };
    own.checked_add(extra)
        .ok_or("descriptor footprint overflow".into())
}
