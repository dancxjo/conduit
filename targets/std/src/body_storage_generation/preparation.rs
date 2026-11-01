//! Prepare exact storage descriptors and their finite implementation bounds.
use super::{
    census::StorageCensus, BodyStorageGenerationEvidence, BodyStorageGenerationRequest,
    SelectedBodyStorageGeneration,
};
use crate::body_storage::SelectedBodyStorage;
use conduit_core as c;

pub(super) fn prepare(
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
        let offer = conduit_std_offers::state_value_durable_std_offer_with_bound(
            &selected.type_name,
            &selected.value_type,
            &selected.default_value,
            selected.binding.maximum_value_bytes,
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
pub(super) fn snapshot_name(host: &c::HostId, reference: &c::BoundedResourceRef) -> String {
    conduit_linux_snapshot_store::snapshot_storage_key(host, reference)
        .trim_start_matches("resource/")
        .into()
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
