//! Trusted, read-only scoped namespace boundary; not an installed semantic Back.
//! Root preparation supplies the descriptor and current authority. Native code
//! in this process remains cooperative; this API is not a syscall sandbox.
mod admission;
mod directory;

use conduit_core::*;
use std::fs::File;

pub const LIST_CALL: &str = "conduit.host/scoped-namespace-list@1";
pub const OPEN_CALL: &str = "conduit.host/scoped-namespace-open-read@1";

#[derive(Debug, Clone, Copy)]
pub struct NamespaceLimits {
    pub maximum_entries: u16,
    pub maximum_name_bytes: u32,
    pub maximum_read_bytes: u32,
    pub maximum_capabilities: u16,
    pub maximum_handles: u16,
    pub maximum_operations: u32,
}

/// Descriptive membership, never possession or a content generation.
#[derive(Debug, PartialEq, Eq)]
pub struct NamespaceEntry {
    pub entry_id: ResourceEntryId,
    pub display_name: String,
}

#[derive(Debug)]
pub enum NamespaceRefusal {
    InvalidBounds,
    Unavailable,
    WrongSelection,
    WrongRoot,
    UnsupportedOperation,
    EntryBound,
    NameBound,
    UnsupportedName,
    UnknownEntry,
    ChangedEntry,
    NotRegularFile,
    ByteBound,
    Capability(BaseCapabilityRefusal),
    Io(std::io::ErrorKind),
}
impl From<std::io::Error> for NamespaceRefusal {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.kind())
    }
}
impl From<BaseCapabilityRefusal> for NamespaceRefusal {
    fn from(error: BaseCapabilityRefusal) -> Self {
        Self::Capability(error)
    }
}

/// Owns exactly one retained root and a finite preparation-time membership
/// snapshot. Replacement provider/Boot requires a new owner and fresh issuer.
/// No public descriptor, path, file handle or clone operation is exposed.
pub struct NamespaceCapability {
    handle: BaseCapabilityHandle,
    maximum_result_bytes: u32,
    claim: BaseOperationClaim,
}

pub struct ScopedNamespace {
    directory: directory::Directory,
    entries: Vec<directory::Entry>,
    listing: Vec<NamespaceEntry>,
    binding: BaseProviderBinding,
    host: HostId,
    boot: BootId,
    root: ResourcePoolId,
    generation: ResourceGenerationId,
    limits: NamespaceLimits,
    capabilities: BaseCapabilityTable,
    retired: bool,
}
impl ScopedNamespace {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        root_descriptor: File,
        host: HostId,
        boot: BootId,
        binding: BaseProviderBinding,
        root: ResourcePoolId,
        generation: ResourceGenerationId,
        limits: NamespaceLimits,
        issuer_key: [u8; 32],
    ) -> Result<Self, NamespaceRefusal> {
        if root.as_str().is_empty()
            || generation.0.is_empty()
            || limits.maximum_entries == 0
            || limits.maximum_name_bytes == 0
            || limits.maximum_read_bytes == 0
            || limits.maximum_capabilities == 0
            || limits.maximum_operations == 0
            || limits
                .maximum_entries
                .checked_add(3)
                .is_none_or(|required| limits.maximum_handles < required)
        {
            return Err(NamespaceRefusal::InvalidBounds);
        }
        let capabilities = BaseCapabilityTable::new(
            host.clone(),
            boot.clone(),
            binding.provider_instance_id.clone(),
            binding.provider_generation,
            issuer_key,
            limits.maximum_capabilities,
        )?;
        let directory = directory::Directory::prepare(root_descriptor)?;
        let provider_generation = binding.provider_generation.to_string();
        let entries = directory.snapshot(
            limits,
            &[
                host.as_str(),
                boot.as_str(),
                binding.provider_instance_id.as_str(),
                &provider_generation,
                root.as_str(),
                &generation.0,
            ],
        )?;
        let listing = entries
            .iter()
            .map(|entry| NamespaceEntry {
                entry_id: entry.id.clone(),
                display_name: entry.name.to_string_lossy().into_owned(),
            })
            .collect();
        Ok(Self {
            directory,
            entries,
            listing,
            binding,
            host,
            boot,
            root,
            generation,
            limits,
            capabilities,
            retired: false,
        })
    }

    /// Trusted admission only: authority must come from the current Host owner,
    /// independently of the proposed Plan. Descriptive Plan IDs grant nothing.
    pub fn issue(
        &mut self,
        request: CapabilityIssueRequest,
        fragment: &PlanFragment,
        placement: &PlacementId,
    ) -> Result<NamespaceCapability, NamespaceRefusal> {
        if self.retired {
            return Err(NamespaceRefusal::Unavailable);
        }
        self.validate_admission(&request, fragment, placement)?;
        let maximum_result_bytes = request.scope.maximum_result_bytes;
        let scope = &request.scope;
        let claim = BaseOperationClaim {
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            plan_id: scope.plan_id.clone(),
            active_play_id: scope.active_play_id.clone(),
            implementation_id: scope.implementation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            parameter_bytes: 0,
            work_units: 0,
        };
        Ok(NamespaceCapability {
            handle: self.capabilities.issue(request)?,
            maximum_result_bytes,
            claim,
        })
    }

    pub fn list<'a>(
        &'a mut self,
        handle: &mut NamespaceCapability,
        claim: &BaseOperationClaim,
    ) -> Result<&'a [NamespaceEntry], NamespaceRefusal> {
        self.validate_operation(claim, LIST_CALL)?;
        let bytes = self
            .listing
            .iter()
            .try_fold(0u32, |sum, entry| {
                sum.checked_add(entry.entry_id.0.len() as u32)?
                    .checked_add(entry.display_name.len() as u32)
            })
            .ok_or(NamespaceRefusal::ByteBound)?;
        if bytes > handle.maximum_result_bytes {
            return Err(NamespaceRefusal::ByteBound);
        }
        handle.check_claim(claim)?;
        handle.claim.parameter_bytes = 0;
        handle.claim.work_units = u64::from(bytes.max(1));
        let lease = self.capabilities.authorize(&handle.handle, &handle.claim)?;
        self.capabilities
            .complete(&mut handle.handle, lease, bytes)?;
        Ok(&self.listing)
    }

    /// Open the exact prepared direct child and read into caller-admitted
    /// storage. Membership identity is checked again; content is a live read,
    /// with no immutable-content, atomic-read or durability promise.
    pub fn open_read(
        &mut self,
        handle: &mut NamespaceCapability,
        claim: &BaseOperationClaim,
        entry: &ResourceEntryId,
        output: &mut [u8],
    ) -> Result<usize, NamespaceRefusal> {
        self.validate_operation(claim, OPEN_CALL)?;
        let selected = self
            .entries
            .iter()
            .find(|item| item.id == *entry)
            .ok_or(NamespaceRefusal::UnknownEntry)?;
        let bound = self
            .limits
            .maximum_read_bytes
            .min(handle.maximum_result_bytes);
        handle.check_claim(claim)?;
        handle.claim.parameter_bytes = entry.0.len() as u32;
        handle.claim.work_units = u64::from(bound.max(1));
        let lease = self.capabilities.authorize(&handle.handle, &handle.claim)?;
        let result = self.directory.read(selected, output, bound);
        // Terminal I/O refusals consume one operation and release the lease.
        // Never publish partial bytes on refusal; scrub the caller's buffer.
        let count = result.as_ref().copied().unwrap_or(0);
        let completion = self
            .capabilities
            .complete(&mut handle.handle, lease, count as u32);
        if result.is_err() || completion.is_err() {
            let touched = output.len().min(bound as usize);
            output[..touched].fill(0);
        }
        completion?;
        result
    }

    /// Terminal loss: restoring availability requires a fresh owner and issuer.
    pub fn retire(&mut self) {
        self.retired = true;
    }

    pub fn revoke_play(&mut self, play: &ActivePlayId) {
        self.capabilities.revoke_play(play);
    }

    fn validate_operation(
        &self,
        claim: &BaseOperationClaim,
        operation: &str,
    ) -> Result<(), NamespaceRefusal> {
        if self.retired {
            return Err(NamespaceRefusal::Unavailable);
        }
        if claim.resource_pool_id != self.root || claim.resource_generation_id != self.generation {
            return Err(NamespaceRefusal::WrongRoot);
        }
        if claim.operation_contract_id.as_str() != operation {
            return Err(NamespaceRefusal::UnsupportedOperation);
        }
        Ok(())
    }
}

impl NamespaceCapability {
    fn check_claim(&self, claim: &BaseOperationClaim) -> Result<(), NamespaceRefusal> {
        let expected = &self.claim;
        if claim.host_id != expected.host_id
            || claim.boot_id != expected.boot_id
            || claim.base_instance_id != expected.base_instance_id
            || claim.base_provider_generation != expected.base_provider_generation
            || claim.plan_id != expected.plan_id
            || claim.active_play_id != expected.active_play_id
            || claim.implementation_id != expected.implementation_id
            || claim.operation_contract_id != expected.operation_contract_id
            || claim.subject_kind != expected.subject_kind
            || claim.resource_pool_id != expected.resource_pool_id
            || claim.resource_generation_id != expected.resource_generation_id
            || claim.envelope_id != expected.envelope_id
        {
            return Err(BaseCapabilityRefusal::WrongScope.into());
        }
        Ok(())
    }
}
