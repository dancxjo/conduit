//! Root-owned finite recording equipment, before and after original authority.
//! Recovery reads preserve evidence; they do not admit resources or Start work.
use crate::{
    execution::Policy,
    execution_authority::{Issuer, NativeFacts, Operation},
};
use conduit_core as c;
pub use conduit_resource_snapshot::native_append::AppendNativeIdentity;
use conduit_resource_snapshot::{
    append::{
        PreparedNorRecordResidence, RecordRecovery, RecordResidenceId, RecordResidenceRefusal,
    },
    native_append::{
        AppendCallRefusal, ReadyRecordAppender, APPEND_AUTHORITY, APPEND_KIND, APPEND_OPERATION,
        APPEND_RESULT_BYTES,
    },
    nor::NorStorage,
};
use sha2::{Digest, Sha256};
pub const RESERVED_PREFIX_BYTES: u32 = 8192;
const ORIGINAL_GRANT: &str = "native/record-append/original";
// Exactly one inline owner; no duplicate provider or allocation just to move stages.
#[allow(clippy::large_enum_variant)]
enum Stage<S> {
    Unused(S),
    Recovered {
        identity: AppendNativeIdentity,
        provider: PreparedNorRecordResidence<S>,
    },
    Ready(Option<ReadyRecordAppender<S>>),
}
pub struct Equipment<S> {
    stage: Option<Stage<S>>,
    selected: bool,
    recovery: Option<Result<RecordRecovery, RecordResidenceRefusal>>,
}
#[derive(Debug)]
pub enum Refusal {
    Scope,
    Authority(crate::execution_authority::Refusal),
    Installation(AppendCallRefusal),
}
impl<S: NorStorage> Equipment<S> {
    /// Retain the sole actual owner whether selected, refused or recovered.
    /// Only immutable native kinds select the read-only finite recovery scan.
    /// # Safety
    /// Root owns this exact checked DURABLE storage and independently initialized
    /// identity. It funds this owner, both fixed buffers and the bounded scan
    /// before constructing it; no timing-sensitive operation is in flight. The
    /// same Root must admit the entire finite residence and exclude physical
    /// timing activity during every later program using NativeActivity. These
    /// original owners must never be replaced by metadata received in a Plan.
    pub unsafe fn new(
        storage: S,
        identity: AppendNativeIdentity,
        body: &conduit_body::BodyId,
        native_kinds: &[(u16, &str)],
        prefix_bytes: u32,
        slots: u32,
    ) -> Self {
        let selected = native_kinds.iter().any(|(_, kind)| *kind == APPEND_KIND);
        if !selected {
            return Self {
                stage: Some(Stage::Unused(storage)),
                selected,
                recovery: None,
            };
        }
        let mut hash = Sha256::new();
        hash.update(b"conduit.root/nor-record-residence@1");
        for value in [body.as_str(), identity.host_id.as_str()] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        let residence = RecordResidenceId(hash.finalize().into());
        let prepared = if body.as_str().is_empty() || prefix_bytes < RESERVED_PREFIX_BYTES {
            Err((storage, RecordResidenceRefusal::Geometry))
        } else {
            PreparedNorRecordResidence::prepare(storage, residence, prefix_bytes, slots)
        };
        match prepared {
            Err((storage, refusal)) => Self {
                stage: Some(Stage::Unused(storage)),
                selected,
                recovery: Some(Err(refusal)),
            },
            Ok(mut provider) => {
                let recovery = Some(provider.recover());
                Self {
                    stage: Some(Stage::Recovered { identity, provider }),
                    selected,
                    recovery,
                }
            }
        }
    }
    pub fn selected(&self) -> bool {
        self.selected
    }
    /// Retains every gap/damage count or exact refusal; never a delivery ACK.
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
    /// Borrow the same initialized owner for descriptive current-Boot matching.
    /// This creates no operation capability and cannot withdraw the provider.
    pub fn ready(&self) -> Option<&ReadyRecordAppender<S>> {
        match self.stage.as_ref()? {
            Stage::Ready(Some(owner)) => Some(owner),
            _ => None,
        }
    }
    pub fn ready_slot(&mut self) -> Option<&mut Option<ReadyRecordAppender<S>>> {
        match self.stage.as_mut()? {
            Stage::Ready(slot) => Some(slot),
            _ => None,
        }
    }
    pub fn publish(
        &self,
        publication: &mut crate::native_images::Publication<'_>,
    ) -> Result<(), crate::native_images::Refusal> {
        use crate::native_images::Refusal as E;
        if !self.selected {
            return Ok(());
        }
        match self.stage.as_ref() {
            Some(Stage::Recovered { identity, provider }) => {
                // SAFETY: new retained this exact original physical pair; no
                // mutable access or replacement is exposed before installation.
                unsafe { publication.recovered_record_append(identity, provider) }
            }
            Some(Stage::Ready(Some(owner))) => publication.record_append(owner),
            _ => Err(E::Context),
        }
    }
    fn facts(&self) -> Result<NativeFacts<'_>, Refusal> {
        match self.stage.as_ref() {
            Some(Stage::Recovered { identity, provider }) => {
                // SAFETY: the exact original owned pair is consumed by install.
                unsafe { NativeFacts::recovered_append(identity, provider) }
                    .map_err(Refusal::Authority)
            }
            Some(Stage::Ready(Some(owner))) if owner.recovered() => Ok(NativeFacts::append(owner)),
            _ => Err(Refusal::Scope),
        }
    }
    pub fn authorize(
        &mut self,
        context: conduit_body::NativePreparationContext<'_>,
        policy: Policy<'_>,
        issuer: &mut Issuer,
        operation: &Operation<'_>,
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        self.check_operation(operation)?;
        issuer
            .authorize(
                context,
                policy,
                &self.facts()?,
                operation,
                ORIGINAL_GRANT,
                maximum_bytes,
            )
            .map_err(Refusal::Authority)?;
        self.install(issuer, operation.maximum_operations)
    }
    pub fn authorize_continuation(
        &mut self,
        context: crate::continuation_native::ContinuationNativeContext<'_>,
        policy: Policy<'_>,
        issuer: &mut Issuer,
        operation: &Operation<'_>,
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        self.check_operation(operation)?;
        issuer
            .authorize_continuation(
                context,
                policy,
                &self.facts()?,
                operation,
                ORIGINAL_GRANT,
                maximum_bytes,
            )
            .map_err(Refusal::Authority)?;
        self.install(issuer, operation.maximum_operations)
    }
    fn check_operation(&self, op: &Operation<'_>) -> Result<(), Refusal> {
        if !self.selected
            || op.kind != APPEND_KIND
            || op.authority != APPEND_AUTHORITY
            || op.call != APPEND_OPERATION
            || op.maximum_parameter_bytes > 200
            || op.maximum_result_bytes != APPEND_RESULT_BYTES
            || op.maximum_work_units < 6
            || op.maximum_operations == 0
        {
            return Err(Refusal::Scope);
        }
        Ok(())
    }
    fn install(&mut self, issuer: &Issuer, maximum_operations: u32) -> Result<(), Refusal> {
        let original = issuer.grant().ok_or(Refusal::Scope)?;
        if let Some(Stage::Ready(Some(owner))) = self.stage.as_ref() {
            return if owner.original_grant() == original
                && owner.attachment().maximum_operations == maximum_operations
            {
                Ok(())
            } else {
                Err(Refusal::Scope)
            };
        }
        let (identity, provider) = match self.stage.take() {
            Some(Stage::Recovered { identity, provider }) => (identity, provider),
            other => {
                self.stage = other;
                return Err(Refusal::Scope);
            }
        };
        // SAFETY: original Issuer just checked this exact pair. Root's ownership,
        // finite funding and future exclusive-window obligations originate in new.
        match unsafe {
            ReadyRecordAppender::new_retained(identity, provider, original, maximum_operations)
        } {
            Ok(owner) => {
                self.stage = Some(Stage::Ready(Some(owner)));
                Ok(())
            }
            Err((identity, provider, refusal)) => {
                self.stage = Some(Stage::Recovered { identity, provider });
                Err(Refusal::Installation(refusal))
            }
        }
    }
}

#[cfg(test)]
#[path = "recording_equipment_tests.rs"]
mod tests;
