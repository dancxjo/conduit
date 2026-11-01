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
mod clone_backing;
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
        let registry_bytes = self.base_registry.entries().iter().try_fold(
            std::mem::size_of::<c::BaseRegistry>(),
            |n, entry| {
                n.checked_add(clone_backing::count(entry)?)
                    .ok_or_else(|| "storage registry workspace overflow".to_string())
            },
        )?;
        let required = staged
            .evidence
            .preparation_bytes
            .checked_add(registry_bytes as u64)
            .ok_or("storage workspace overflow")?;
        if required > storage.bounds().preparation_bytes {
            return Err(format!(
                "storage generation preparation requires {required} bytes, selected {}",
                storage.bounds().preparation_bytes
            ));
        }
        let mut staged = staged;
        staged.evidence.preparation_bytes = required;
        let mut registry = self.base_registry.clone();
        match old {
            Some(prior) => registry.replace(
                &prior.base.base_id,
                &prior.base.provider_instance_id,
                prior.base.provider_generation,
                staged.base.clone(),
            ),
            None => registry.register(staged.base.clone()),
        }
        .map_err(|error| format!("storage Base selection: {error:?}"))?;
        let mut next = self.advertisement.clone();
        if let Some(prior) = old {
            next.resources.retain(|r| {
                !prior
                    .base
                    .resources
                    .iter()
                    .any(|old| old.pool_id == r.pool_id)
            });
            next.capabilities.retain(|r| {
                !prior
                    .base
                    .capabilities
                    .iter()
                    .any(|old| old.capability_id == r.capability_id)
            });
            next.bases.retain(|r| r.base_id != prior.base.base_id);
        }
        for resource in &staged.base.resources {
            if next.resources.iter().any(|r| r.pool_id == resource.pool_id) {
                return Err("storage pool collides with other equipment".into());
            }
        }
        for capability in &staged.base.capabilities {
            if next
                .capabilities
                .iter()
                .any(|r| r.capability_id == capability.capability_id)
            {
                return Err("storage capability collides with other equipment".into());
            }
        }
        next.offer_generation = generation;
        registry
            .project_base_ready_into(&staged.base.base_id, &mut next)
            .map_err(|error| format!("storage Base projection: {error:?}"))?;
        next.resources.sort();
        crate::normalize_capability_offers(&mut next.capabilities)?;
        let backing = self
            .native_source
            .as_ref()
            .and_then(|source| source.backing.as_ref());
        let ledger = self.kernel_resources.prepare_storage_refresh(
            &self.advertisement,
            &next,
            old.map(|g| &g.base),
            &staged.base,
            backing,
        )?;
        // This preflight performs no mutation, then the established provider
        // generation transfer occurs before the remaining infallible commit.
        self.advance_retained_spoken_offer_generation(generation)?;
        if let Some(backing) = self
            .native_source
            .as_mut()
            .and_then(|source| source.backing.as_mut())
        {
            backing.adopt_refreshed_ledger(&ledger);
        }
        let evidence = staged.evidence.clone();
        Arc::get_mut(self.selected_body_storage.as_mut().expect("selected above"))
            .expect("exclusive holder checked above")
            .generation = Some(staged);
        self.base_registry = registry;
        self.advertisement = next;
        self.kernel_resources = ledger;
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
    // Base mapping plus staged Base registry/advertisement copies and selected
    // resource clones coexist. Count descriptor backing without making a JSON
    // tree or copying immutable capability payloads merely to measure them.
    let descriptor_bound = clone_backing::count(&base)?
        .checked_mul(4)
        .ok_or("storage descriptor workspace overflow")?;
    let inventory_bound = clone_backing::count(host)?;
    let binding_bytes = binding.as_ref().map_or(0, |binding| {
        binding.body.capacity() + binding.state.as_str().len() + binding.value_kind.as_str().len()
    });
    let preparation = storage
        .custody_preparation_bytes()
        .checked_add(
            (std::mem::size_of::<SelectedBodyStorageGeneration>()
                + 2 * std::mem::size_of::<usize>()
                + descriptor_bound
                + inventory_bound
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
