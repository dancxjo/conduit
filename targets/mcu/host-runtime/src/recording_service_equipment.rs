//! Root-owned append/read service over one recovered affine NOR residence.
//! Public inspection and original grant descriptions never authorize Start.
extern crate alloc;
pub use crate::recording_residence::RESERVED_PREFIX_BYTES;
use crate::{
    execution::Policy,
    execution_authority::{Issuer, NativeFacts, Operation},
};
use alloc::string::String;
use conduit_core as c;
pub use conduit_resource_snapshot::native_append::AppendNativeIdentity;
use conduit_resource_snapshot::{
    append::{PreparedNorRecordResidence, RecordRecovery, RecordResidenceRefusal},
    native_append::{APPEND_AUTHORITY, APPEND_KIND, APPEND_OPERATION, APPEND_RESULT_BYTES},
    native_service::{
        ReadyRecordService, ServiceLimits, ServiceRefusal, READ_AUTHORITY, READ_OPERATION,
    },
    nor::NorStorage,
};
const ORIGINALS: [&str; 2] = [
    "native/record-service/append/original",
    "native/record-service/read/original",
];
#[allow(clippy::large_enum_variant)]
enum Stage<S> {
    Unused(S),
    Recovered {
        identity: AppendNativeIdentity,
        provider: PreparedNorRecordResidence<S>,
    },
    Ready(Option<ReadyRecordService<S>>),
}
pub struct Equipment<S> {
    stage: Option<Stage<S>>,
    selected: bool,
    recovery: Option<Result<RecordRecovery, RecordResidenceRefusal>>,
}
#[derive(Debug)]
pub enum Refusal {
    Scope,
    Capacity {
        required: usize,
        maximum: usize,
    },
    Allocation,
    /// Issuers retain independently successful originals if the other refuses.
    /// No Ready service is exposed; retry with the same originals/context.
    Authority {
        operation: usize,
        reason: crate::execution_authority::Refusal,
    },
    Installation(ServiceRefusal),
}
impl<S: NorStorage> Equipment<S> {
    /// # Safety
    /// Root exclusively owns this checked durable physical owner/identity and
    /// funds its finite recovery scan, inline state, codec buffers/metadata and
    /// later legal media windows. Never install an append owner alongside this
    /// service over the same residence. No native timing operation may be active.
    pub unsafe fn new(
        storage: S,
        identity: AppendNativeIdentity,
        body: &conduit_body::BodyId,
        native_kinds: &[(u16, &str)],
        prefix_bytes: u32,
        slots: u32,
    ) -> Self {
        let selected = native_kinds
            .iter()
            .any(|(_, kind)| *kind == conduit_resource_snapshot::service_contract::SERVICE_KIND);
        if !selected {
            return Self {
                stage: Some(Stage::Unused(storage)),
                selected,
                recovery: None,
            };
        }
        match crate::recording_residence::recover(storage, &identity, body, prefix_bytes, slots) {
            Err((storage, refusal)) => Self {
                stage: Some(Stage::Unused(storage)),
                selected,
                recovery: Some(Err(refusal)),
            },
            Ok((provider, recovery)) => Self {
                stage: Some(Stage::Recovered { identity, provider }),
                selected,
                recovery: Some(recovery),
            },
        }
    }
    pub fn selected(&self) -> bool {
        self.selected
    }
    pub fn recovery(&self) -> Option<&Result<RecordRecovery, RecordResidenceRefusal>> {
        self.recovery.as_ref()
    }
    pub fn unused_storage(&self) -> Option<&S> {
        match self.stage.as_ref()? {
            Stage::Unused(storage) => Some(storage),
            _ => None,
        }
    }
    pub fn held_pool(&self) -> Option<&c::ResourcePoolId> {
        match self.stage.as_ref()? {
            Stage::Recovered { identity, .. } => Some(&identity.resource_pool_id),
            Stage::Ready(Some(owner)) => Some(&owner.native_identity().resource_pool_id),
            _ => None,
        }
    }
    /// Describe only this same recovered/Ready residence. The caller retains
    /// and funds this descriptor before assigned preparation; inline storage and
    /// allocator overhead are separate. It proves no resource admission, grant
    /// or permission to withdraw the owner through the original Start boundary.
    pub fn resource_binding(
        &self,
        maximum_bytes: usize,
    ) -> Result<Option<c::ResourceBinding>, Refusal> {
        if !self.selected {
            return Ok(None);
        }
        self.facts()?;
        let pool = self.held_pool().ok_or(Refusal::Scope)?.as_str();
        let class = conduit_resource_snapshot::native_append::APPEND_CLASS;
        let required = pool
            .len()
            .checked_add(class.len())
            .ok_or(Refusal::Capacity {
                required: usize::MAX,
                maximum: maximum_bytes,
            })?;
        if required > maximum_bytes {
            return Err(Refusal::Capacity {
                required,
                maximum: maximum_bytes,
            });
        }
        let mut remaining = maximum_bytes;
        let mut copy = |text: &str| -> Result<String, Refusal> {
            let used = maximum_bytes - remaining;
            if text.len() > remaining {
                return Err(Refusal::Capacity {
                    required: used.saturating_add(text.len()),
                    maximum: maximum_bytes,
                });
            }
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| Refusal::Allocation)?;
            remaining = remaining
                .checked_sub(owned.capacity())
                .ok_or(Refusal::Capacity {
                    required: used.saturating_add(owned.capacity()),
                    maximum: maximum_bytes,
                })?;
            owned.push_str(text);
            Ok(owned)
        };
        Ok(Some(c::ResourceBinding {
            pool_id: copy(pool)?.into(),
            class_id: copy(class)?.into(),
            units: 1,
            protected: None,
            compute: None,
            content: None,
        }))
    }
    pub fn ready(&self) -> Option<&ReadyRecordService<S>> {
        match self.stage.as_ref()? {
            Stage::Ready(Some(owner)) => Some(owner),
            _ => None,
        }
    }
    pub fn ready_slot(&mut self) -> Option<&mut Option<ReadyRecordService<S>>> {
        match self.stage.as_mut()? {
            Stage::Ready(slot) => Some(slot),
            _ => None,
        }
    }
    pub fn publish(
        &self,
        publication: &mut crate::native_images::Publication<'_>,
    ) -> Result<(), crate::native_images::Refusal> {
        if !self.selected {
            return Ok(());
        }
        match self.stage.as_ref() {
            // SAFETY: constructor retains this exact recovered original pair.
            Some(Stage::Recovered { identity, provider }) => unsafe {
                publication.recovered_record_service(identity, provider)
            },
            Some(Stage::Ready(Some(owner))) => publication.record_service(owner),
            _ => Err(crate::native_images::Refusal::Context),
        }
    }
    fn facts(&self) -> Result<NativeFacts<'_>, Refusal> {
        match self.stage.as_ref() {
            Some(Stage::Recovered { identity, provider }) => {
                // SAFETY: installation consumes this same original pair.
                unsafe { NativeFacts::recovered_record_service(identity, provider) }.map_err(
                    |reason| Refusal::Authority {
                        operation: 0,
                        reason,
                    },
                )
            }
            Some(Stage::Ready(Some(owner))) if owner.remaining_slots().is_some() => {
                Ok(NativeFacts::record_service(owner))
            }
            _ => Err(Refusal::Scope),
        }
    }
    /// Append then read, each with its independently retained original Issuer.
    /// maximum_bytes bounds each original description and the combined copies;
    /// Root separately funds Ready codec allocations and preparation overhead.
    /// A later refusal preserves earlier originals for an explicit exact retry.
    pub fn authorize(
        &mut self,
        context: conduit_body::NativePreparationContext<'_>,
        policy: Policy<'_>,
        issuers: [&mut Issuer; 2],
        operations: [&Operation<'_>; 2],
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        self.check_operations(operations)?;
        let [append, read] = issuers;
        for (i, issuer) in [(&mut *append), (&mut *read)].into_iter().enumerate() {
            issuer
                .authorize(
                    context,
                    policy,
                    &self.facts()?,
                    operations[i],
                    ORIGINALS[i],
                    maximum_bytes,
                )
                .map_err(|reason| Refusal::Authority {
                    operation: i,
                    reason,
                })?;
        }
        self.install([append, read], operations, maximum_bytes)
    }
    pub fn authorize_continuation(
        &mut self,
        context: crate::continuation_native::ContinuationNativeContext<'_>,
        policy: Policy<'_>,
        issuers: [&mut Issuer; 2],
        operations: [&Operation<'_>; 2],
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        self.check_operations(operations)?;
        let [append, read] = issuers;
        for (i, issuer) in [(&mut *append), (&mut *read)].into_iter().enumerate() {
            issuer
                .authorize_continuation(
                    context,
                    policy,
                    &self.facts()?,
                    operations[i],
                    ORIGINALS[i],
                    maximum_bytes,
                )
                .map_err(|reason| Refusal::Authority {
                    operation: i,
                    reason,
                })?;
        }
        self.install([append, read], operations, maximum_bytes)
    }
    fn check_operations(&self, operations: [&Operation<'_>; 2]) -> Result<(), Refusal> {
        let [append, read] = operations;
        if !self.selected
            || append.kind != APPEND_KIND
            || append.authority != APPEND_AUTHORITY
            || append.call != APPEND_OPERATION
            || append.maximum_parameter_bytes > 200
            || append.maximum_result_bytes != APPEND_RESULT_BYTES
            || append.maximum_work_units < 6
            || append.maximum_operations == 0
            || read.kind != conduit_resource_snapshot::read_contract::READ_KIND
            || read.authority != READ_AUTHORITY
            || read.call != READ_OPERATION
            || read.maximum_parameter_bytes
                != conduit_resource_snapshot::read_contract::READ_INPUT_BYTES
            || read.maximum_result_bytes
                != conduit_resource_snapshot::read_result::READ_RESULT_BYTES as u32
            || read.maximum_work_units
                < conduit_resource_snapshot::read_contract::MAXIMUM_MEDIA_READS_PER_REQUEST
            || read.maximum_operations == 0
        {
            return Err(Refusal::Scope);
        }
        Ok(())
    }
    fn install(
        &mut self,
        issuers: [&Issuer; 2],
        operations: [&Operation<'_>; 2],
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        let limits = ServiceLimits {
            append_operations: operations[0].maximum_operations,
            read_operations: operations[1].maximum_operations,
        };
        let originals = [
            issuers[0].grant().ok_or(Refusal::Scope)?,
            issuers[1].grant().ok_or(Refusal::Scope)?,
        ];
        if let Some(Stage::Ready(Some(owner))) = self.stage.as_ref() {
            return if owner
                .original_grants()
                .iter()
                .zip(originals)
                .all(|(a, b)| a == b)
                && owner.limits() == limits
            {
                Ok(())
            } else {
                Err(Refusal::Scope)
            };
        }
        let mut remaining = maximum_bytes;
        let append = issuers[0]
            .copy_original_grant(&mut remaining)
            .map_err(|reason| Refusal::Authority {
                operation: 0,
                reason,
            })?;
        let read = issuers[1]
            .copy_original_grant(&mut remaining)
            .map_err(|reason| Refusal::Authority {
                operation: 1,
                reason,
            })?;
        let (identity, provider) = match self.stage.take() {
            Some(Stage::Recovered { identity, provider }) => (identity, provider),
            other => {
                self.stage = other;
                return Err(Refusal::Scope);
            }
        };
        // SAFETY: same exclusively owned recovered residence, independently
        // original grants and Root-funded finite limits/buffers/windows.
        match unsafe {
            ReadyRecordService::new_retained(identity, provider, [append, read], limits)
        } {
            Ok(owner) => {
                self.stage = Some(Stage::Ready(Some(owner)));
                Ok(())
            }
            Err((identity, provider, _, refusal)) => {
                self.stage = Some(Stage::Recovered { identity, provider });
                Err(Refusal::Installation(refusal))
            }
        }
    }
}
#[cfg(test)]
#[path = "recording_service_equipment_tests.rs"]
mod tests;
