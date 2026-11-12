//! Cold construction of one independently selected immutable Text residence.
//! Metadata is intent; the actual directory and original grant are supplied
//! separately. No disk capacity, membership or execution authority is inferred.
use crate::{text_file::SelectedTextFileResidence, StdHost, StdHostConfig};
use conduit_core::*;
use conduit_data::DataReference;
use conduit_host_make::HostImage;
use conduit_linux_snapshot_store::SnapshotDirectory;
use conduit_std_offers::*;
use std::{fs::File, os::unix::fs::MetadataExt};

/// Descriptive equipment correspondence. Possessing this object is neither a
/// Ready provider nor permission to open a directory or execute an operation.
pub struct NativeTextFileDefinition {
    pub base: BaseProviderEntry,
    pub authority: AuthorityGrant,
    pub maximum_text_bytes: u32,
    pub preparation_bytes: u32,
    pub retained_bytes: u32,
}
/// Affine Root inputs to checked cold construction, never accepted from a Plan.
pub struct TextFileInitialization {
    pub definition: NativeTextFileDefinition,
    pub directory: File,
    pub reference: BoundedResourceRef,
}
pub(crate) struct InitialTextFile {
    pub definition: NativeTextFileDefinition,
    directory: SnapshotDirectory,
    reference: BoundedResourceRef,
    root_identity: (u64, u64),
}
impl InitialTextFile {
    pub(crate) fn prepare(
        config: &StdHostConfig,
        image: &HostImage,
        input: TextFileInitialization,
    ) -> Result<Self, String> {
        validate_definition(
            &input.definition,
            &input.reference,
            &config.host_id,
            &config.boot_id,
        )?;
        let metadata = input
            .directory
            .metadata()
            .map_err(|error| format!("selected Text directory: {error}"))?;
        let root_identity = (metadata.dev(), metadata.ino());
        let selected = Self {
            definition: input.definition,
            directory: SnapshotDirectory::prepare(input.directory)
                .map_err(|error| format!("selected Text directory: {error}"))?,
            reference: input.reference,
            root_identity,
        };
        selected.validate_image(image)?;
        Ok(selected)
    }
    pub(crate) fn owns_pool(&self, id: &str) -> bool {
        self.definition
            .base
            .resources
            .iter()
            .any(|pool| pool.pool_id.as_str() == id)
    }
    pub(crate) fn validate_image(&self, image: &HostImage) -> Result<(), String> {
        let d = &self.definition;
        let mut selected = image
            .manifest
            .base_selections
            .iter()
            .filter(|base| base.id.as_str() == d.base.base_id.as_str());
        let base = selected
            .next()
            .ok_or("IMAGE lacks the selected Text file Base")?;
        if selected.next().is_some()
            || base.kind != DURABLE_TEXT_BASE_KIND
            || base.kind != d.base.mechanism_family.as_str()
            || base.driver != d.base.implementation_id.as_str()
            || image
                .manifest
                .implementations
                .iter()
                .filter(|implementation| implementation.as_str() == base.driver)
                .count()
                != 1
        {
            return Err("selected Text file Base differs from original IMAGE equipment".into());
        }
        for resource in &d.base.resources {
            let mut budgets = image
                .manifest
                .resource_budgets
                .iter()
                .filter(|budget| budget.id == resource.pool_id.as_str());
            let budget = budgets
                .next()
                .ok_or("IMAGE lacks the Text file resource pool")?;
            if budgets.next().is_some()
                || budget.class != resource.class_id.as_str()
                || budget.slots != resource.capacity_units
                || budget.bytes < u64::from(d.maximum_text_bytes)
            {
                return Err("selected Text file pool differs from original IMAGE budget".into());
            }
        }
        if u64::from(d.preparation_bytes) > image.manifest.bounds.heap_arena_bytes {
            return Err("Text file preparation exceeds original IMAGE heap".into());
        }
        Ok(())
    }
    pub(crate) fn initialize(self, host: &mut StdHost) -> Result<(), String> {
        validate_definition(
            &self.definition,
            &self.reference,
            &host.advertisement.host_id,
            &host.advertisement.boot_id,
        )?;
        if host.selected_text_file.is_some()
            || !host
                .base_registry
                .entries()
                .iter()
                .any(|base| base == &self.definition.base)
            || !self
                .definition
                .base
                .resources
                .iter()
                .all(|pool| host.advertisement.resources.contains(pool))
            || !host
                .advertisement
                .capabilities
                .contains(&self.definition.base.capabilities[0])
        {
            return Err(
                "Text file owner differs from staged Host or duplicates a residence".into(),
            );
        }
        let d = self.definition;
        let metadata = crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes();
        let preparation = (d.preparation_bytes as usize)
            .checked_add(metadata)
            .ok_or("Text file preparation size overflow")?;
        let retention = (d.retained_bytes as usize)
            .checked_add(metadata)
            .ok_or("Text file retention size overflow")?;
        let mut heap = host
            .kernel_resources
            .reserve_managed_base_memory(preparation)?;
        let mut selected = SelectedTextFileResidence {
            directory: self.directory,
            reference: self.reference,
            root_identity: self.root_identity,
            base_id: d.base.base_id,
            authority_grant: Some(d.authority),
            image_memory: None,
        };
        let prepared = selected
            .retained_backing_bytes()
            .ok_or("Text file owner backing overflow")
            .and_then(|required| {
                if required > d.retained_bytes as usize {
                    Err("Text file owner exceeds its original retained budget")
                } else {
                    Ok(())
                }
            });
        prepared.map_err(str::to_owned)?;
        heap.retain_result(retention);
        selected.image_memory = Some(heap);
        host.selected_text_file = Some(selected);
        Ok(())
    }
}

fn validate_definition(
    d: &NativeTextFileDefinition,
    reference: &BoundedResourceRef,
    host: &HostId,
    boot: &BootId,
) -> Result<(), String> {
    DataReference::new(reference.clone())
        .and_then(|value| value.validate_for(&kind_id("value/text")))
        .map_err(|error| format!("selected immutable Text reference: {error:?}"))?;
    if d.maximum_text_bytes == 0
        || d.maximum_text_bytes > DURABLE_TEXT_MAXIMUM_BYTES
        || reference.extent.bytes > u64::from(d.maximum_text_bytes)
        || d.preparation_bytes == 0
        || d.retained_bytes == 0
        || d.retained_bytes > d.preparation_bytes
        || d.base.lifecycle != BaseLifecycle::Ready
        || d.base.enforcement_class != BaseEnforcementClass::Cooperative
        || d.base.provider_instance_id.as_str().is_empty()
        || d.base.provider_generation == 0
        || d.base.base_id.as_str().is_empty()
        || d.base.mechanism_family.as_str() != DURABLE_TEXT_BASE_KIND
        || d.base.capabilities.len() != 1
        || d.base.resources.len() != 1
    {
        return Err("Text file requires one finite independently initialized provider".into());
    }
    let content = d.base.resources[0]
        .content
        .as_ref()
        .ok_or("Text file lacks selected content residence")?;
    if d.base.resources[0].pool_id.as_str().is_empty()
        || d.base.resources[0].capacity_units != 1
        || d.base.resources[0].class_id != reference.access_class
        || d.base.resources[0].compute.is_some()
        || content.owner_host != *host
        || content.owner_boot != *boot
        || content.base_id != d.base.base_id
        || content.residence_profile.as_str() != DURABLE_TEXT_RESIDENCE
        || content.contract.identity != reference.identity
        || content.contract.version != reference.lifetime.version
        || content.contract.content_profile != reference.content_profile
        || content.contract.maximum_bytes != d.maximum_text_bytes
    {
        return Err("Text file residence differs from exact resource, reference or Host".into());
    }
    let expected = durable_text_offer(content.contract.clone())?;
    let grant = &d.authority;
    if d.base.capabilities[0] != expected
        || d.base.implementation_id != expected.implementation.implementation_id
        || grant.grant_id.as_str().is_empty()
        || grant.host_id != *host
        || grant.boot_id != *boot
        || grant.capability_id != expected.capability_id
        || grant.subject_kind != expected.kind_id
        || grant.contract_id.as_str() != DURABLE_TEXT_AUTHORITY
        || grant.host_call_contract_id != expected.host_calls[0].contract_id
    {
        return Err("Text file definition lacks original current operation authority".into());
    }
    Ok(())
}

impl StdHost {
    /// Borrow original independently supplied Root authority; a description or
    /// matching resource reference alone cannot create this grant.
    pub fn text_file_authority_grant(&self) -> Option<&AuthorityGrant> {
        self.selected_text_file
            .as_ref()
            .and_then(|selected| selected.authority_grant.as_ref())
    }
}

#[cfg(test)]
mod tests;
