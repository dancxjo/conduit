use std::collections::BTreeSet;
use std::fmt;

use conduit_core::Id;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::PATCHBAY_PROTOCOL_VERSION;

pub const MAXIMUM_RESOURCE_BINDING_SLOTS: usize = 32;
pub const MAXIMUM_RESOURCE_BINDING_RECEIPTS: usize = 64;
pub const MAXIMUM_PENDING_RESOURCE_SELECTIONS: usize = 32;
pub const MAXIMUM_RESOURCE_BINDING_TEXT_BYTES: usize = 1_024;
pub const MAXIMUM_RESOURCE_BINDING_SCOPES: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingPrincipal {
    User,
    Site,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceSelectionOperation {
    Enumerate,
    Choose,
    CreateNew,
    ReplaceExisting,
    SelectContainer,
    DownloadExport,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceAccessScope {
    Read,
    Write,
    Replace,
    Create,
    Enumerate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceSelectionAccessProfile {
    pub operation: ResourceSelectionOperation,
    pub access: Vec<ResourceAccessScope>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionProviderKind {
    DeterministicMemory,
    BrowserFile,
    HostedLocalBroker,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionProviderState {
    Available,
    Unsupported,
    Disappeared,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBindingSlot {
    pub id: String,
    pub resource_reference: String,
    pub grant_reference: String,
    pub resource_kind: String,
    pub required_profile: String,
    pub principal: BindingPrincipal,
    pub allowed_selection: Vec<ResourceSelectionOperation>,
    pub selection_access: Vec<ResourceSelectionAccessProfile>,
    /// Other slot identities which must not resolve to the same protected
    /// resource. This is a semantic binding constraint (for example, Copy's
    /// source must not also be its destination), not a comparison of labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disallow_same_resource_as: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionProviderObservation {
    pub id: String,
    pub host_id: String,
    pub observation_id: String,
    pub generation: u64,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub kind: SelectionProviderKind,
    pub state: SelectionProviderState,
    pub resource_kind: String,
    pub supported_operations: Vec<ResourceSelectionOperation>,
    pub enumeration_authorized: bool,
}

impl SelectionProviderObservation {
    #[must_use]
    pub fn deterministic_files(now: u64) -> Self {
        file_provider(
            "conduit.selector/deterministic-files",
            "conduit/deterministic-host",
            "conduit.selector-observation/deterministic-files",
            SelectionProviderKind::DeterministicMemory,
            &[
                ResourceSelectionOperation::Enumerate,
                ResourceSelectionOperation::Choose,
                ResourceSelectionOperation::CreateNew,
                ResourceSelectionOperation::ReplaceExisting,
                ResourceSelectionOperation::SelectContainer,
            ],
            true,
            now,
        )
    }

    #[must_use]
    pub fn browser_files(now: u64) -> Self {
        file_provider(
            "conduit.selector/browser-files",
            "conduit/browser-worker",
            "conduit.selector-observation/browser-files",
            SelectionProviderKind::BrowserFile,
            &[
                ResourceSelectionOperation::Choose,
                ResourceSelectionOperation::CreateNew,
                ResourceSelectionOperation::ReplaceExisting,
                ResourceSelectionOperation::DownloadExport,
            ],
            false,
            now,
        )
    }

    #[must_use]
    pub fn hosted_local_files(now: u64) -> Self {
        file_provider(
            "conduit.selector/hosted-local-files",
            "conduit/hosted-local",
            "conduit.selector-observation/hosted-local-files",
            SelectionProviderKind::HostedLocalBroker,
            &[
                ResourceSelectionOperation::Choose,
                ResourceSelectionOperation::CreateNew,
                ResourceSelectionOperation::ReplaceExisting,
                ResourceSelectionOperation::SelectContainer,
            ],
            false,
            now,
        )
    }

    #[must_use]
    pub fn unsupported_files(now: u64) -> Self {
        file_provider(
            "conduit.selector/unsupported-files",
            "conduit/unsupported-host",
            "conduit.selector-observation/unsupported-files",
            SelectionProviderKind::Unsupported,
            &[],
            false,
            now,
        )
    }
}

fn file_provider(
    id: &str,
    host_id: &str,
    observation_id: &str,
    kind: SelectionProviderKind,
    supported_operations: &[ResourceSelectionOperation],
    enumeration_authorized: bool,
    now: u64,
) -> SelectionProviderObservation {
    SelectionProviderObservation {
        id: id.to_owned(),
        host_id: host_id.to_owned(),
        observation_id: observation_id.to_owned(),
        generation: 1,
        observed_at_tick: now,
        valid_until_tick: now.saturating_add(100),
        kind,
        state: if kind == SelectionProviderKind::Unsupported {
            SelectionProviderState::Unsupported
        } else {
            SelectionProviderState::Available
        },
        resource_kind: "conduit.resource/filesystem-file".to_owned(),
        supported_operations: supported_operations.to_vec(),
        enumeration_authorized,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceBindingRequestAction {
    Enumerate,
    Choose,
    CreateNew,
    ReplaceExisting,
    SelectContainer,
    DownloadExport,
    Cancel,
    ConfirmGrant,
    Inspect,
    Revoke,
    Forget,
}

impl ResourceBindingRequestAction {
    const fn selection_operation(self) -> Option<ResourceSelectionOperation> {
        match self {
            Self::Enumerate => Some(ResourceSelectionOperation::Enumerate),
            Self::Choose => Some(ResourceSelectionOperation::Choose),
            Self::CreateNew => Some(ResourceSelectionOperation::CreateNew),
            Self::ReplaceExisting => Some(ResourceSelectionOperation::ReplaceExisting),
            Self::SelectContainer => Some(ResourceSelectionOperation::SelectContainer),
            Self::DownloadExport => Some(ResourceSelectionOperation::DownloadExport),
            Self::Cancel | Self::ConfirmGrant | Self::Inspect | Self::Revoke | Self::Forget => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBindingRequestEnvelope {
    pub protocol_version: u16,
    pub request_id: String,
    pub operation_id: String,
    pub slot_id: String,
    pub expected_binding_revision: u64,
    pub provider_id: String,
    pub provider_generation: u64,
    pub action: ResourceBindingRequestAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requested_access: Vec<ResourceAccessScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_request_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBindingReceipt {
    pub sequence: u64,
    pub request_id: String,
    pub operation_id: String,
    pub slot_id: String,
    pub binding_revision: u64,
    pub action: ResourceBindingRequestAction,
    pub disposition: String,
    pub code: String,
    pub explanation: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionCompletionOutcome {
    Selected,
    Cancelled,
    PermissionDenied,
    ResourceDisappeared,
    ProviderDisappeared,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProtectedValue(String);

impl ProtectedValue {
    pub fn new(value: impl Into<String>) -> Result<Self, ResourceBindingError> {
        let value = value.into();
        if !bounded_id(&value) {
            return Err(ResourceBindingError::Malformed);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn expose_for_exact_resolution(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProtectedValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedSelectionCompletion {
    pub request_id: String,
    pub operation_id: String,
    pub provider_id: String,
    pub provider_observation_id: String,
    pub provider_generation: u64,
    pub completed_at_tick: u64,
    pub outcome: SelectionCompletionOutcome,
    pub opaque_handle: Option<ProtectedValue>,
    pub safe_label: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantConfirmationOutcome {
    Granted,
    Denied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedGrantConfirmation {
    pub slot_id: String,
    pub expected_binding_revision: u64,
    pub authority_observation_id: String,
    pub authority_generation: u64,
    pub outcome: GrantConfirmationOutcome,
    pub grant: Option<ProtectedValue>,
    pub access: Vec<ResourceAccessScope>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingSelection {
    request_id: String,
    operation_id: String,
    slot_id: String,
    provider_id: String,
    provider_observation_id: String,
    provider_generation: u64,
    operation: ResourceSelectionOperation,
    requested_access: Vec<ResourceAccessScope>,
    expected_binding_revision: u64,
    cancelled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProtectedResourceBinding {
    slot_id: String,
    revision: u64,
    provider_id: String,
    provider_observation_id: String,
    provider_generation: u64,
    provider_kind: SelectionProviderKind,
    provider_state: SelectionProviderState,
    opaque_handle: ProtectedValue,
    safe_label: String,
    platform_permission: String,
    grant_state: String,
    grant: Option<ProtectedValue>,
    authority_observation_id: Option<String>,
    authority_generation: Option<u64>,
    access: Vec<ResourceAccessScope>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceBindingSlotProjection {
    pub id: String,
    pub resource_kind: String,
    pub required_profile: String,
    pub principal: BindingPrincipal,
    pub binding_revision: u64,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<SelectionProviderKind>,
    pub provider_state: String,
    pub platform_permission: String,
    pub conduit_grant: String,
    pub allowed_selection: Vec<ResourceSelectionOperation>,
    pub required_access: Vec<ResourceAccessScope>,
    pub selection_access: Vec<ResourceSelectionAccessProfile>,
    pub available_actions: Vec<ResourceBindingRequestAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explanations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProtectedBindingProfileProjection {
    pub profile_id: String,
    pub revision: u64,
    pub identity: String,
    pub slots: Vec<ResourceBindingSlotProjection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<ResourceBindingReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedBindingExportPolicy {
    RedactedSafeMetadata,
    Refuse,
}

pub struct ExactResourceBindingResolution<'a> {
    pub slot_id: &'a str,
    pub binding_revision: u64,
    pub provider_id: &'a str,
    pub provider_observation_id: &'a str,
    pub provider_generation: u64,
    resource: &'a ProtectedValue,
    grant: &'a ProtectedValue,
    pub access: &'a [ResourceAccessScope],
}

impl<'a> ExactResourceBindingResolution<'a> {
    #[must_use]
    pub fn resource(&self) -> &'a str {
        &self.resource.0
    }

    #[must_use]
    pub fn grant(&self) -> &'a str {
        &self.grant.0
    }
}

impl fmt::Debug for ExactResourceBindingResolution<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactResourceBindingResolution")
            .field("slot_id", &self.slot_id)
            .field("binding_revision", &self.binding_revision)
            .field("provider_id", &self.provider_id)
            .field("provider_observation_id", &self.provider_observation_id)
            .field("provider_generation", &self.provider_generation)
            .field("resource", &"[REDACTED]")
            .field("grant", &"[REDACTED]")
            .field("access", &self.access)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceBindingError {
    Malformed,
    DuplicateSlot,
    TooManySlots,
    UnknownSlot,
    WrongRevision,
    WrongProvider,
    StaleProvider,
    Unsupported,
    EnumerationDenied,
    AccessDenied,
    DuplicateRequest,
    UnknownRequest,
    CancelledRequest,
    PermissionDenied,
    ResourceDisappeared,
    ProviderDisappeared,
    GrantRequired,
    GrantDenied,
    ProtectedExportRefused,
    Capacity,
}

impl ResourceBindingError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Malformed => "CND-BND-001",
            Self::DuplicateSlot => "CND-BND-002",
            Self::TooManySlots => "CND-BND-003",
            Self::UnknownSlot => "CND-BND-004",
            Self::WrongRevision | Self::StaleProvider => "CND-BND-005",
            Self::WrongProvider => "CND-BND-006",
            Self::Unsupported | Self::EnumerationDenied => "CND-BND-007",
            Self::AccessDenied | Self::GrantDenied => "CND-BND-008",
            Self::DuplicateRequest | Self::UnknownRequest | Self::CancelledRequest => "CND-BND-009",
            Self::PermissionDenied => "CND-BND-010",
            Self::ResourceDisappeared | Self::ProviderDisappeared => "CND-BND-011",
            Self::GrantRequired => "CND-BND-012",
            Self::ProtectedExportRefused => "CND-BND-013",
            Self::Capacity => "CND-BND-003",
        }
    }
}

#[derive(Clone)]
pub struct ProtectedBindingProfile {
    profile_id: String,
    revision: u64,
    slots: Vec<ResourceBindingSlot>,
    bindings: Vec<ProtectedResourceBinding>,
    pending: Vec<PendingSelection>,
    receipts: Vec<ResourceBindingReceipt>,
    next_sequence: u64,
}

impl ProtectedBindingProfile {
    pub fn new(
        profile_id: impl Into<String>,
        slots: Vec<ResourceBindingSlot>,
    ) -> Result<Self, ResourceBindingError> {
        let profile_id = profile_id.into();
        if !bounded_id(&profile_id)
            || slots.is_empty()
            || slots.len() > MAXIMUM_RESOURCE_BINDING_SLOTS
        {
            return Err(if slots.len() > MAXIMUM_RESOURCE_BINDING_SLOTS {
                ResourceBindingError::TooManySlots
            } else {
                ResourceBindingError::Malformed
            });
        }
        let mut ids = BTreeSet::new();
        let mut resource_references = BTreeSet::new();
        let mut grant_references = BTreeSet::new();
        for slot in &slots {
            let mut operations = BTreeSet::new();
            let mut access_operations = BTreeSet::new();
            let mut conflicts = BTreeSet::new();
            if !bounded_id(&slot.id)
                || !bounded_id(&slot.resource_reference)
                || !bounded_id(&slot.grant_reference)
                || !bounded_id(&slot.resource_kind)
                || !bounded_id(&slot.required_profile)
                || slot.allowed_selection.is_empty()
                || slot.allowed_selection.len() > MAXIMUM_RESOURCE_BINDING_SCOPES
                || slot.selection_access.len() != slot.allowed_selection.len()
                || slot.disallow_same_resource_as.len() > MAXIMUM_RESOURCE_BINDING_SLOTS
                || !ids.insert(slot.id.clone())
                || !resource_references.insert(slot.resource_reference.clone())
                || !grant_references.insert(slot.grant_reference.clone())
                || slot
                    .allowed_selection
                    .iter()
                    .any(|value| !operations.insert(*value))
                || slot.selection_access.iter().any(|profile| {
                    let unique = profile.access.iter().copied().collect::<BTreeSet<_>>();
                    profile.access.is_empty()
                        || profile.access.len() > MAXIMUM_RESOURCE_BINDING_SCOPES
                        || unique.len() != profile.access.len()
                        || !operations.contains(&profile.operation)
                        || !access_operations.insert(profile.operation)
                })
                || slot.disallow_same_resource_as.iter().any(|value| {
                    value == &slot.id || !bounded_id(value) || !conflicts.insert(value.clone())
                })
            {
                return Err(ResourceBindingError::DuplicateSlot);
            }
        }
        if slots.iter().any(|slot| {
            slot.disallow_same_resource_as
                .iter()
                .any(|other| !ids.contains(other))
        }) {
            return Err(ResourceBindingError::UnknownSlot);
        }
        Ok(Self {
            profile_id,
            revision: 0,
            slots,
            bindings: Vec::new(),
            pending: Vec::new(),
            receipts: Vec::new(),
            next_sequence: 1,
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.slots.is_empty() && self.slots.iter().all(|slot| self.resolve(&slot.id).is_ok())
    }

    pub fn resolve_resource_reference(
        &self,
        resource_reference: &str,
    ) -> Result<&str, ResourceBindingError> {
        let slot = self
            .slots
            .iter()
            .find(|slot| slot.resource_reference == resource_reference)
            .ok_or(ResourceBindingError::UnknownSlot)?;
        self.resolve(&slot.id)
            .map(|resolution| resolution.resource())
    }

    pub fn resolve_grant_reference(
        &self,
        grant_reference: &str,
    ) -> Result<&str, ResourceBindingError> {
        let slot = self
            .slots
            .iter()
            .find(|slot| slot.grant_reference == grant_reference)
            .ok_or(ResourceBindingError::UnknownSlot)?;
        self.resolve(&slot.id).map(|resolution| resolution.grant())
    }

    pub fn begin_selection(
        &mut self,
        request: &ResourceBindingRequestEnvelope,
        provider: &SelectionProviderObservation,
        now: u64,
    ) -> Result<ResourceBindingReceipt, ResourceBindingError> {
        self.validate_request(request)?;
        self.ensure_fresh_request(&request.request_id)?;
        if self.pending.len() >= MAXIMUM_PENDING_RESOURCE_SELECTIONS {
            if let Some(cancelled) = self.pending.iter().position(|pending| pending.cancelled) {
                self.pending.remove(cancelled);
            } else {
                return Err(ResourceBindingError::Capacity);
            }
        }
        let slot = self.slot(&request.slot_id)?.clone();
        let operation = request
            .action
            .selection_operation()
            .ok_or(ResourceBindingError::Unsupported)?;
        if request.expected_binding_revision != self.binding_revision(&slot.id) {
            return Err(ResourceBindingError::WrongRevision);
        }
        validate_provider(&slot, provider, request, operation, now)?;
        let required_access = slot
            .selection_access
            .iter()
            .find(|profile| profile.operation == operation)
            .map(|profile| profile.access.as_slice())
            .ok_or(ResourceBindingError::Unsupported)?;
        if !same_set(required_access, &request.requested_access) {
            return Err(ResourceBindingError::AccessDenied);
        }
        let pending = PendingSelection {
            request_id: request.request_id.clone(),
            operation_id: request.operation_id.clone(),
            slot_id: slot.id.clone(),
            provider_id: provider.id.clone(),
            provider_observation_id: provider.observation_id.clone(),
            provider_generation: provider.generation,
            operation,
            requested_access: request.requested_access.clone(),
            expected_binding_revision: request.expected_binding_revision,
            cancelled: false,
        };
        self.pending.push(pending);
        Ok(self.push_receipt(
            request,
            self.binding_revision(&slot.id),
            "selection-pending",
            "CND-BND-PENDING",
            "The exact user-mediated selection ceremony is pending; no binding or grant exists yet.",
        ))
    }

    pub fn cancel_selection(
        &mut self,
        request: &ResourceBindingRequestEnvelope,
    ) -> Result<ResourceBindingReceipt, ResourceBindingError> {
        self.validate_request(request)?;
        self.ensure_fresh_request(&request.request_id)?;
        if request.action != ResourceBindingRequestAction::Cancel {
            return Err(ResourceBindingError::Unsupported);
        }
        let target = request
            .selection_request_id
            .as_deref()
            .ok_or(ResourceBindingError::Malformed)?;
        let pending_index = self
            .pending
            .iter()
            .position(|pending| pending.request_id == target)
            .ok_or(ResourceBindingError::UnknownRequest)?;
        let pending = &self.pending[pending_index];
        if pending.cancelled {
            return Err(ResourceBindingError::CancelledRequest);
        }
        if self.binding_revision(&pending.slot_id) != pending.expected_binding_revision {
            self.pending.remove(pending_index);
            return Err(ResourceBindingError::WrongRevision);
        }
        if pending.slot_id != request.slot_id || pending.operation_id != request.operation_id {
            return Err(ResourceBindingError::WrongProvider);
        }
        self.pending[pending_index].cancelled = true;
        Ok(self.push_receipt(
            request,
            self.binding_revision(&request.slot_id),
            "cancelled",
            "CND-BND-CANCELLED",
            "The selection ceremony was cancelled; a later provider callback cannot create a binding.",
        ))
    }

    pub fn complete_selection(
        &mut self,
        completion: ProtectedSelectionCompletion,
        provider: &SelectionProviderObservation,
    ) -> Result<ResourceBindingReceipt, ResourceBindingError> {
        let index = self
            .pending
            .iter()
            .position(|pending| pending.request_id == completion.request_id)
            .ok_or(ResourceBindingError::UnknownRequest)?;
        let pending = self.pending[index].clone();
        if pending.cancelled {
            return Err(ResourceBindingError::CancelledRequest);
        }
        if self.binding_revision(&pending.slot_id) != pending.expected_binding_revision {
            self.pending.remove(index);
            return Err(ResourceBindingError::WrongRevision);
        }
        if pending.operation_id != completion.operation_id
            || pending.provider_id != completion.provider_id
            || pending.provider_observation_id != completion.provider_observation_id
            || pending.provider_generation != completion.provider_generation
            || provider.id != completion.provider_id
            || provider.observation_id != completion.provider_observation_id
            || provider.generation != completion.provider_generation
        {
            return Err(ResourceBindingError::WrongProvider);
        }
        if provider.state != SelectionProviderState::Available {
            self.pending.remove(index);
            return Err(ResourceBindingError::ProviderDisappeared);
        }
        if completion.completed_at_tick < provider.observed_at_tick
            || completion.completed_at_tick > provider.valid_until_tick
        {
            self.pending.remove(index);
            return Err(ResourceBindingError::StaleProvider);
        }
        let slot_id = pending.slot_id.clone();
        let action = selection_action(pending.operation);
        let synthetic_request = ResourceBindingRequestEnvelope {
            protocol_version: PATCHBAY_PROTOCOL_VERSION,
            request_id: completion.request_id.clone(),
            operation_id: completion.operation_id.clone(),
            slot_id: slot_id.clone(),
            expected_binding_revision: self.binding_revision(&slot_id),
            provider_id: completion.provider_id.clone(),
            provider_generation: completion.provider_generation,
            action,
            requested_access: pending.requested_access.clone(),
            selection_request_id: None,
        };
        match completion.outcome {
            SelectionCompletionOutcome::Cancelled => {
                self.pending.remove(index);
                Ok(self.push_receipt(
                    &synthetic_request,
                    self.binding_revision(&slot_id),
                    "cancelled",
                    "CND-BND-CANCELLED",
                    "The provider reported that the user cancelled selection.",
                ))
            }
            SelectionCompletionOutcome::PermissionDenied => {
                self.pending.remove(index);
                Err(ResourceBindingError::PermissionDenied)
            }
            SelectionCompletionOutcome::ResourceDisappeared => {
                self.pending.remove(index);
                Err(ResourceBindingError::ResourceDisappeared)
            }
            SelectionCompletionOutcome::ProviderDisappeared => {
                self.pending.remove(index);
                Err(ResourceBindingError::ProviderDisappeared)
            }
            SelectionCompletionOutcome::Selected => {
                let handle = completion
                    .opaque_handle
                    .ok_or(ResourceBindingError::Malformed)?;
                let label = completion
                    .safe_label
                    .ok_or(ResourceBindingError::Malformed)?;
                if !safe_label(&label) {
                    return Err(ResourceBindingError::Malformed);
                }
                let slot = self.slot(&slot_id)?;
                if self.bindings.iter().any(|binding| {
                    slot.disallow_same_resource_as.contains(&binding.slot_id)
                        && binding.opaque_handle == handle
                }) {
                    self.pending.remove(index);
                    return Err(ResourceBindingError::AccessDenied);
                }
                self.revision = self.revision.saturating_add(1);
                let binding_revision = self.revision;
                let binding = ProtectedResourceBinding {
                    slot_id: slot_id.clone(),
                    revision: binding_revision,
                    provider_id: provider.id.clone(),
                    provider_observation_id: provider.observation_id.clone(),
                    provider_generation: provider.generation,
                    provider_kind: provider.kind,
                    provider_state: provider.state,
                    opaque_handle: handle,
                    safe_label: label,
                    platform_permission: "granted".to_owned(),
                    grant_state: "required".to_owned(),
                    grant: None,
                    authority_observation_id: None,
                    authority_generation: None,
                    access: pending.requested_access,
                };
                if let Some(existing) = self
                    .bindings
                    .iter_mut()
                    .find(|binding| binding.slot_id == slot_id)
                {
                    *existing = binding;
                } else {
                    self.bindings.push(binding);
                }
                self.pending.remove(index);
                Ok(self.push_receipt(
                    &synthetic_request,
                    binding_revision,
                    "selected-grant-required",
                    "CND-BND-012",
                    "The protected resource selection was stored, but Conduit authority remains separately required.",
                ))
            }
        }
    }

    pub fn confirm_grant(
        &mut self,
        request: &ResourceBindingRequestEnvelope,
        confirmation: ProtectedGrantConfirmation,
    ) -> Result<ResourceBindingReceipt, ResourceBindingError> {
        self.validate_request(request)?;
        self.ensure_fresh_request(&request.request_id)?;
        if request.action != ResourceBindingRequestAction::ConfirmGrant
            || request.slot_id != confirmation.slot_id
            || request.expected_binding_revision != confirmation.expected_binding_revision
            || !same_set(&request.requested_access, &confirmation.access)
        {
            return Err(ResourceBindingError::Malformed);
        }
        let binding = self
            .bindings
            .iter_mut()
            .find(|binding| binding.slot_id == request.slot_id)
            .ok_or(ResourceBindingError::GrantRequired)?;
        if binding.revision != confirmation.expected_binding_revision {
            return Err(ResourceBindingError::WrongRevision);
        }
        if !same_set(&binding.access, &confirmation.access) {
            return Err(ResourceBindingError::AccessDenied);
        }
        self.revision = self.revision.saturating_add(1);
        binding.revision = self.revision;
        binding.authority_observation_id = Some(confirmation.authority_observation_id);
        binding.authority_generation = Some(confirmation.authority_generation);
        let (disposition, code, explanation) = match confirmation.outcome {
            GrantConfirmationOutcome::Granted => {
                binding.grant = Some(confirmation.grant.ok_or(ResourceBindingError::Malformed)?);
                binding.grant_state = "granted".to_owned();
                (
                    "ready",
                    "CND-BND-READY",
                    "The protected binding and separately authorized Conduit grant are ready for candidate-plan resolution.",
                )
            }
            GrantConfirmationOutcome::Denied => {
                binding.grant = None;
                binding.grant_state = "denied".to_owned();
                (
                    "grant-denied",
                    "CND-BND-008",
                    "Conduit authority was denied; OS selection remains distinct and cannot authorize execution.",
                )
            }
        };
        Ok(self.push_receipt(request, self.revision, disposition, code, explanation))
    }

    pub fn revoke(
        &mut self,
        request: &ResourceBindingRequestEnvelope,
    ) -> Result<ResourceBindingReceipt, ResourceBindingError> {
        self.validate_request(request)?;
        self.ensure_fresh_request(&request.request_id)?;
        if request.action != ResourceBindingRequestAction::Revoke {
            return Err(ResourceBindingError::Unsupported);
        }
        let binding = self
            .bindings
            .iter_mut()
            .find(|binding| binding.slot_id == request.slot_id)
            .ok_or(ResourceBindingError::UnknownSlot)?;
        if binding.revision != request.expected_binding_revision {
            return Err(ResourceBindingError::WrongRevision);
        }
        self.revision = self.revision.saturating_add(1);
        binding.revision = self.revision;
        binding.grant = None;
        binding.grant_state = "revoked".to_owned();
        Ok(self.push_receipt(
            request,
            self.revision,
            "revoked",
            "CND-BND-REVOKED",
            "Conduit authority was revoked; the selected resource remains protected but cannot resolve into a candidate plan.",
        ))
    }

    pub fn inspect(
        &mut self,
        request: &ResourceBindingRequestEnvelope,
    ) -> Result<ResourceBindingReceipt, ResourceBindingError> {
        self.validate_request(request)?;
        self.ensure_fresh_request(&request.request_id)?;
        if request.action != ResourceBindingRequestAction::Inspect {
            return Err(ResourceBindingError::Unsupported);
        }
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.slot_id == request.slot_id)
            .ok_or(ResourceBindingError::UnknownSlot)?;
        if binding.revision != request.expected_binding_revision {
            return Err(ResourceBindingError::WrongRevision);
        }
        Ok(self.push_receipt(
            request,
            binding.revision,
            "inspected",
            "CND-BND-INSPECTED",
            "Returned the bounded safe binding projection; exact resource and grant material remain protected.",
        ))
    }

    pub fn forget(
        &mut self,
        request: &ResourceBindingRequestEnvelope,
    ) -> Result<ResourceBindingReceipt, ResourceBindingError> {
        self.validate_request(request)?;
        self.ensure_fresh_request(&request.request_id)?;
        if request.action != ResourceBindingRequestAction::Forget {
            return Err(ResourceBindingError::Unsupported);
        }
        let index = self
            .bindings
            .iter()
            .position(|binding| binding.slot_id == request.slot_id)
            .ok_or(ResourceBindingError::UnknownSlot)?;
        if self.bindings[index].revision != request.expected_binding_revision {
            return Err(ResourceBindingError::WrongRevision);
        }
        self.bindings.remove(index);
        self.revision = self.revision.saturating_add(1);
        Ok(self.push_receipt(
            request,
            0,
            "forgotten",
            "CND-BND-FORGOTTEN",
            "The protected binding was forgotten; shared source and presentation identity were unchanged.",
        ))
    }

    pub fn reconcile_provider(&mut self, provider: &SelectionProviderObservation, now: u64) {
        let mut changed = false;
        for binding in self
            .bindings
            .iter_mut()
            .filter(|binding| binding.provider_id == provider.id)
        {
            let next = if provider.state == SelectionProviderState::Available
                && provider.observation_id == binding.provider_observation_id
                && provider.generation == binding.provider_generation
                && provider.valid_until_tick >= now
            {
                SelectionProviderState::Available
            } else {
                SelectionProviderState::Disappeared
            };
            if binding.provider_state != next {
                binding.provider_state = next;
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.saturating_add(1);
            for binding in self
                .bindings
                .iter_mut()
                .filter(|binding| binding.provider_id == provider.id)
            {
                binding.revision = self.revision;
            }
        }
    }

    pub fn resolve(
        &self,
        slot_id: &str,
    ) -> Result<ExactResourceBindingResolution<'_>, ResourceBindingError> {
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.slot_id == slot_id)
            .ok_or(ResourceBindingError::GrantRequired)?;
        if binding.provider_state != SelectionProviderState::Available {
            return Err(ResourceBindingError::ProviderDisappeared);
        }
        if binding.platform_permission != "granted" {
            return Err(ResourceBindingError::PermissionDenied);
        }
        let grant = binding
            .grant
            .as_ref()
            .filter(|_| binding.grant_state == "granted")
            .ok_or(if binding.grant_state == "denied" {
                ResourceBindingError::GrantDenied
            } else {
                ResourceBindingError::GrantRequired
            })?;
        Ok(ExactResourceBindingResolution {
            slot_id: &binding.slot_id,
            binding_revision: binding.revision,
            provider_id: &binding.provider_id,
            provider_observation_id: &binding.provider_observation_id,
            provider_generation: binding.provider_generation,
            resource: &binding.opaque_handle,
            grant,
            access: &binding.access,
        })
    }

    #[must_use]
    pub fn projection(&self) -> ProtectedBindingProfileProjection {
        let slots = self
            .slots
            .iter()
            .map(|slot| {
                let binding = self
                    .bindings
                    .iter()
                    .find(|binding| binding.slot_id == slot.id);
                let pending = self
                    .pending
                    .iter()
                    .find(|pending| pending.slot_id == slot.id && !pending.cancelled);
                let (state, explanations) = if pending.is_some() {
                    (
                        "selection-pending",
                        vec!["A user-mediated chooser is open; it has not changed the protected binding.".to_owned()],
                    )
                } else if let Some(binding) = binding {
                    if binding.provider_state != SelectionProviderState::Available {
                        (
                            "provider-disappeared",
                            vec!["The provider observation no longer validates this opaque handle.".to_owned()],
                        )
                    } else if binding.grant_state != "granted" {
                        (
                            binding.grant_state.as_str(),
                            vec!["Resource selection is not Conduit authority; confirm the exact scope separately.".to_owned()],
                        )
                    } else {
                        ("ready", Vec::new())
                    }
                } else {
                    (
                        "selection-required",
                        vec!["Choose a resource through an authorized typed selection provider.".to_owned()],
                    )
                };
                let mut available_actions = if let Some(pending) = pending {
                    vec![
                        selection_action(pending.operation),
                        ResourceBindingRequestAction::Cancel,
                    ]
                } else {
                    slot.allowed_selection
                        .iter()
                        .copied()
                        .map(selection_action)
                        .collect::<Vec<_>>()
                };
                if pending.is_none() && binding.is_some() {
                    available_actions.push(ResourceBindingRequestAction::Inspect);
                    if binding.is_some_and(|binding| binding.grant_state != "granted") {
                        available_actions.push(ResourceBindingRequestAction::ConfirmGrant);
                    } else {
                        available_actions.push(ResourceBindingRequestAction::Revoke);
                    }
                    available_actions.push(ResourceBindingRequestAction::Forget);
                }
                let required_access = binding.map_or_else(
                    || {
                        slot.selection_access
                            .iter()
                            .flat_map(|profile| profile.access.iter().copied())
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect()
                    },
                    |binding| binding.access.clone(),
                );
                ResourceBindingSlotProjection {
                    id: slot.id.clone(),
                    resource_kind: slot.resource_kind.clone(),
                    required_profile: slot.required_profile.clone(),
                    principal: slot.principal,
                    binding_revision: binding.map_or(0, |binding| binding.revision),
                    state: state.to_owned(),
                    pending_request_id: pending.map(|pending| pending.request_id.clone()),
                    pending_operation_id: pending.map(|pending| pending.operation_id.clone()),
                    safe_label: binding.map(|binding| binding.safe_label.clone()),
                    provider_kind: binding.map(|binding| binding.provider_kind),
                    provider_state: binding.map_or("not-observed", |binding| match binding.provider_state {
                        SelectionProviderState::Available => "available",
                        SelectionProviderState::Unsupported => "unsupported",
                        SelectionProviderState::Disappeared => "disappeared",
                    }).to_owned(),
                    platform_permission: binding.map_or("not-requested", |binding| binding.platform_permission.as_str()).to_owned(),
                    conduit_grant: binding.map_or("not-requested", |binding| binding.grant_state.as_str()).to_owned(),
                    allowed_selection: slot.allowed_selection.clone(),
                    required_access,
                    selection_access: slot.selection_access.clone(),
                    available_actions,
                    explanations,
                }
            })
            .collect::<Vec<_>>();
        let mut identity_input = format!(
            "conduit.protected-binding-profile\0{}\0{}\0",
            self.profile_id, self.revision
        );
        for slot in &slots {
            identity_input.push_str(&format!(
                "{}\0{}\0{}\0",
                slot.id, slot.binding_revision, slot.state
            ));
        }
        ProtectedBindingProfileProjection {
            profile_id: self.profile_id.clone(),
            revision: self.revision,
            identity: format!("sha256:{:x}", Sha256::digest(identity_input.as_bytes())),
            slots,
            receipts: self.receipts.clone(),
        }
    }

    pub fn export_projection(
        &self,
        policy: ProtectedBindingExportPolicy,
    ) -> Result<ProtectedBindingProfileProjection, ResourceBindingError> {
        match policy {
            ProtectedBindingExportPolicy::RedactedSafeMetadata => Ok(self.projection()),
            ProtectedBindingExportPolicy::Refuse => {
                Err(ResourceBindingError::ProtectedExportRefused)
            }
        }
    }

    fn validate_request(
        &self,
        request: &ResourceBindingRequestEnvelope,
    ) -> Result<(), ResourceBindingError> {
        if request.protocol_version != PATCHBAY_PROTOCOL_VERSION
            || !bounded_id(&request.request_id)
            || !bounded_id(&request.operation_id)
            || !bounded_id(&request.slot_id)
            || (!request.provider_id.is_empty() && !bounded_id(&request.provider_id))
            || request.requested_access.len() > MAXIMUM_RESOURCE_BINDING_SCOPES
            || request
                .requested_access
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != request.requested_access.len()
        {
            return Err(ResourceBindingError::Malformed);
        }
        self.slot(&request.slot_id)?;
        Ok(())
    }

    fn slot(&self, slot_id: &str) -> Result<&ResourceBindingSlot, ResourceBindingError> {
        self.slots
            .iter()
            .find(|slot| slot.id == slot_id)
            .ok_or(ResourceBindingError::UnknownSlot)
    }

    fn ensure_fresh_request(&self, request_id: &str) -> Result<(), ResourceBindingError> {
        if self
            .receipts
            .iter()
            .any(|receipt| receipt.request_id == request_id)
            || self
                .pending
                .iter()
                .any(|pending| pending.request_id == request_id)
        {
            Err(ResourceBindingError::DuplicateRequest)
        } else {
            Ok(())
        }
    }

    fn binding_revision(&self, slot_id: &str) -> u64 {
        self.bindings
            .iter()
            .find(|binding| binding.slot_id == slot_id)
            .map_or(0, |binding| binding.revision)
    }

    fn push_receipt(
        &mut self,
        request: &ResourceBindingRequestEnvelope,
        binding_revision: u64,
        disposition: &str,
        code: &str,
        explanation: &str,
    ) -> ResourceBindingReceipt {
        let receipt = ResourceBindingReceipt {
            sequence: self.next_sequence,
            request_id: request.request_id.clone(),
            operation_id: request.operation_id.clone(),
            slot_id: request.slot_id.clone(),
            binding_revision,
            action: request.action,
            disposition: disposition.to_owned(),
            code: code.to_owned(),
            explanation: explanation.to_owned(),
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.receipts.push(receipt.clone());
        if self.receipts.len() > MAXIMUM_RESOURCE_BINDING_RECEIPTS {
            self.receipts.remove(0);
        }
        receipt
    }
}

fn validate_provider(
    slot: &ResourceBindingSlot,
    provider: &SelectionProviderObservation,
    request: &ResourceBindingRequestEnvelope,
    operation: ResourceSelectionOperation,
    now: u64,
) -> Result<(), ResourceBindingError> {
    if request.provider_id != provider.id || request.provider_generation != provider.generation {
        return Err(ResourceBindingError::WrongProvider);
    }
    if provider.valid_until_tick < now || provider.observed_at_tick > now {
        return Err(ResourceBindingError::StaleProvider);
    }
    match provider.state {
        SelectionProviderState::Unsupported => return Err(ResourceBindingError::Unsupported),
        SelectionProviderState::Disappeared => {
            return Err(ResourceBindingError::ProviderDisappeared);
        }
        SelectionProviderState::Available => {}
    }
    if provider.resource_kind != slot.resource_kind
        || !slot.allowed_selection.contains(&operation)
        || !provider.supported_operations.contains(&operation)
    {
        return Err(ResourceBindingError::Unsupported);
    }
    if operation == ResourceSelectionOperation::Enumerate && !provider.enumeration_authorized {
        return Err(ResourceBindingError::EnumerationDenied);
    }
    Ok(())
}

fn selection_action(operation: ResourceSelectionOperation) -> ResourceBindingRequestAction {
    match operation {
        ResourceSelectionOperation::Enumerate => ResourceBindingRequestAction::Enumerate,
        ResourceSelectionOperation::Choose => ResourceBindingRequestAction::Choose,
        ResourceSelectionOperation::CreateNew => ResourceBindingRequestAction::CreateNew,
        ResourceSelectionOperation::ReplaceExisting => {
            ResourceBindingRequestAction::ReplaceExisting
        }
        ResourceSelectionOperation::SelectContainer => {
            ResourceBindingRequestAction::SelectContainer
        }
        ResourceSelectionOperation::DownloadExport => ResourceBindingRequestAction::DownloadExport,
    }
}

fn same_set<T: Ord + Copy>(left: &[T], right: &[T]) -> bool {
    left.len() == right.len()
        && left.iter().copied().collect::<BTreeSet<_>>()
            == right.iter().copied().collect::<BTreeSet<_>>()
}

fn bounded_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAXIMUM_RESOURCE_BINDING_TEXT_BYTES
        && Id::new(value).is_ok()
}

fn safe_label(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAXIMUM_RESOURCE_BINDING_TEXT_BYTES
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains("..")
        && !value.chars().any(char::is_control)
}
