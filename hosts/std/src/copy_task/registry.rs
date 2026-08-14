use conduit_core::{
    BootId, CapabilityId, GearId, HostId, ProtectedResourceAccess, ProtectedResourceCommitPolicy,
    ProtectedResourceGrant, ResourceBindingRoleId, ResourceClassId, ResourceHandleId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedFileAvailability {
    Available,
    Denied,
}

#[derive(Debug, Clone)]
pub(crate) struct ProtectedFileEntry {
    pub(crate) path: PathBuf,
    pub(crate) grant: ProtectedResourceGrant,
    pub(crate) availability: ProtectedFileAvailability,
}

#[derive(Debug, Default)]
pub struct ProtectedFileRegistry {
    entries: BTreeMap<ResourceHandleId, ProtectedFileEntry>,
    retired_handles: BTreeSet<ResourceHandleId>,
}

impl ProtectedFileRegistry {
    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &mut self,
        handle_id: ResourceHandleId,
        path: impl AsRef<Path>,
        gear_id: GearId,
        role_id: ResourceBindingRoleId,
        host_id: HostId,
        boot_id: BootId,
        capability_id: CapabilityId,
        access: ProtectedResourceAccess,
        maximum_bytes: u64,
        commit_policy: ProtectedResourceCommitPolicy,
        availability: ProtectedFileAvailability,
    ) -> Result<ProtectedResourceGrant, String> {
        if maximum_bytes == 0
            || self.entries.contains_key(&handle_id)
            || self.retired_handles.contains(&handle_id)
        {
            return Err(
                "protected file registration must have a unique handle and positive bound"
                    .to_string(),
            );
        }
        let grant = ProtectedResourceGrant {
            role_id,
            handle_id: handle_id.clone(),
            gear_id,
            host_id,
            boot_id,
            capability_id,
            class_id: ResourceClassId::from(conduit_std_catalog::PROTECTED_FILE_RESOURCE_CLASS),
            access,
            maximum_bytes,
            commit_policy,
        };
        self.entries.insert(
            handle_id,
            ProtectedFileEntry {
                path: path.as_ref().to_path_buf(),
                grant: grant.clone(),
                availability,
            },
        );
        Ok(grant)
    }

    pub fn revoke(&mut self, handle_id: &ResourceHandleId) {
        if self.entries.remove(handle_id).is_some() {
            self.retired_handles.insert(handle_id.clone());
        }
    }

    pub fn set_availability(
        &mut self,
        handle_id: &ResourceHandleId,
        availability: ProtectedFileAvailability,
    ) -> Result<(), String> {
        let entry = self
            .entries
            .get_mut(handle_id)
            .ok_or_else(|| "protected file handle is not current".to_string())?;
        entry.availability = availability;
        Ok(())
    }

    pub(crate) fn get(&self, handle_id: &ResourceHandleId) -> Option<&ProtectedFileEntry> {
        self.entries.get(handle_id)
    }
}
