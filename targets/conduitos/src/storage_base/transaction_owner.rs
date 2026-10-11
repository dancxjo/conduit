//! Independent storage possession around two actual selected Host Calls.
//! No USB class policy or transport completion inference lives here.
use super::{contract::*, request::*, transaction_contract::*};
use crate::machine_membrane::selected_operation::*;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use sha2::{Digest, Sha256};
pub(crate) mod retention;

pub(crate) struct StorageScopeAdmission<'a> {
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
    pub selected: SelectedOperationPlan<'a>,
}
struct Scope {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
}
/// Actual move-only possession retained by the original subordinate dispatcher.
/// Request/ordinal are inspectable correlation; they cannot construct this token.
pub(crate) struct StorageAuthorizationSubmission {
    lease: BaseOperationLease,
    request: BlockReadRequest,
    ordinal: u32,
    preparation: [u8; 32],
}
impl StorageAuthorizationSubmission {
    pub(crate) fn request(&self) -> BlockReadRequest {
        self.request
    }
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn preparation(&self) -> &[u8; 32] {
        &self.preparation
    }
}
/// Constructor is restricted to audited in-crate native composition; serialized
/// candidate bytes, capability inspection or successful Source output cannot mint it.
pub(crate) struct StorageSettlementReceipt(StorageAuthorizationSubmission);
pub(crate) struct StorageCancellationReceipt(StorageAuthorizationSubmission);
impl StorageCancellationReceipt {
    pub(crate) fn into_submission(self) -> StorageAuthorizationSubmission {
        self.0
    }
    /// # Safety
    /// Original subordinate association must acknowledge physical stop and
    /// kernel retirement for this exact retained submission; idle is insufficient.
    pub(crate) unsafe fn stopped(submission: StorageAuthorizationSubmission) -> Self {
        Self(submission)
    }
}
impl StorageSettlementReceipt {
    pub(crate) fn into_submission(self) -> StorageAuthorizationSubmission {
        self.0
    }
    /// # Safety
    /// The original preparation-owned exclusive association must have verified
    /// native continuity and zero physical pending DMA/leases/uncertainty AND
    /// retirement of every subordinate kernel obligation for this exact token.
    /// Cancellation additionally requires actual acknowledged native stop.
    /// A zero capability-table counter or Source verdict is insufficient.
    pub(crate) unsafe fn acknowledged(submission: StorageAuthorizationSubmission) -> Self {
        Self(submission)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum StorageTransactionRefusal {
    Binding,
    Possession,
    Retention,
    Pressure,
    Stale,
    Terminal,
    Candidate,
    Capability(BaseCapabilityRefusal),
    Canonical(StructuredInfoRefusal),
    Request(BlockReadRequestRefusal),
}
pub(crate) struct BlockReadTransactionOwner {
    authorize: Scope,
    settle: Scope,
    decoder: PreparedBlockReadRequestDecoder,
    validator: PreparedStructuredValueValidator,
    permit: PreparedStructuredComposer,
    numbers: [PreparedStructuredComposer; 4],
    pending: Option<(BaseOperationLease, BlockReadRequest, u32)>,
    next: u32,
    next_settle_request: u32,
    preparation: [u8; 32],
    terminal: bool,
    retained_bytes: usize,
}
impl BlockReadTransactionOwner {
    /// Bind two independently issued scopes, exact lowered placements and one
    /// original provider. The dispatcher must retain the exclusive association
    /// before allowing the authorize permit to enable subordinate effects.
    pub(crate) fn bind(
        contract: &BlockReadTransactionContract,
        authorize: StorageScopeAdmission<'_>,
        settle: StorageScopeAdmission<'_>,
    ) -> Result<Self, StorageTransactionRefusal> {
        use StorageTransactionRefusal as E;
        let metadata = retention::metadata(&authorize)?
            .checked_add(retention::metadata(&settle)?)
            .ok_or(E::Retention)?;
        let before = crate::allocation::BOOT_ARENA.live_bytes();
        let a = &authorize.claim;
        let s = &settle.claim;
        if authorize.selected.fragment != settle.selected.fragment
            || a.host_id != s.host_id
            || a.boot_id != s.boot_id
            || a.plan_id != s.plan_id
            || a.active_play_id != s.active_play_id
            || a.base_instance_id != s.base_instance_id
            || a.base_provider_generation != s.base_provider_generation
            || a.base_provider_generation == 0
            || authorize.selected.placement_id == settle.selected.placement_id
        {
            return Err(E::Binding);
        }
        let mut hash = Sha256::new();
        // Bind exact independently issued scope bytes before Play. This digest
        // is correlation inside sealed tokens, never possession or a proof key.
        for admission in [&authorize, &settle] {
            let mut entries = admission.table.inspections();
            let entry = entries.next().ok_or(E::Possession)?;
            if entries.next().is_some()
                || entry.scope.maximum_in_flight != 1
                || entry.scope.maximum_parameter_bytes < BLOCK_READ_MAXIMUM_BYTES
                || entry.scope.maximum_result_bytes < BLOCK_READ_MAXIMUM_BYTES
                || admission.claim.parameter_bytes != BLOCK_READ_MAXIMUM_BYTES
                || admission.claim.work_units != 1
            {
                return Err(E::Possession);
            }
            let bytes = serde_json::to_vec(&entry.scope).map_err(|_| E::Possession)?;
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        let preparation = hash.finalize().into();
        let authorize = bind_scope(authorize, contract.authorize_kind(), AUTHORIZE_CALL)?;
        let settle = bind_scope(settle, contract.settle_kind(), SETTLE_CALL)?;
        let integer = || {
            PreparedStructuredComposer::new(&StructuredInfoType::leaf(kind_id("value/u64"))?, 64)
        };
        let mut owner = Self {
            authorize,
            settle,
            decoder: PreparedBlockReadRequestDecoder::new(contract.block())
                .map_err(E::Canonical)?,
            validator: PreparedStructuredValueValidator::new(
                contract.block().result_type(),
                BLOCK_READ_MAXIMUM_BYTES as usize,
            )
            .map_err(E::Canonical)?,
            permit: PreparedStructuredComposer::new(
                contract.permit_type(),
                BLOCK_READ_MAXIMUM_BYTES as usize,
            )
            .map_err(E::Canonical)?,
            numbers: [
                integer().map_err(E::Canonical)?,
                integer().map_err(E::Canonical)?,
                integer().map_err(E::Canonical)?,
                integer().map_err(E::Canonical)?,
            ],
            pending: None,
            next: 0,
            next_settle_request: 0,
            preparation,
            terminal: false,
            retained_bytes: 0,
        };
        let allocated = crate::allocation::BOOT_ARENA
            .live_bytes()
            .checked_sub(before)
            .ok_or(E::Retention)?;
        owner.retained_bytes = metadata.checked_add(allocated).ok_or(E::Retention)?;
        Ok(owner)
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn authorize_claim(&self) -> &BaseOperationClaim {
        &self.authorize.claim
    }
    pub(crate) fn settle_claim(&self) -> &BaseOperationClaim {
        &self.settle.claim
    }
    pub(crate) fn preparation(&self) -> &[u8; 32] {
        &self.preparation
    }
    pub(crate) fn authorize_node(&self) -> NodeId {
        self.authorize.node
    }
    pub(crate) fn settle_node(&self) -> NodeId {
        self.settle.node
    }
    pub(crate) fn authorize(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        bytes: &[u8],
    ) -> Result<(StorageAuthorizationSubmission, &[u8]), StorageTransactionRefusal> {
        use StorageTransactionRefusal as E;
        if node != self.authorize.node || call != HostCallId(0) {
            return Err(E::Binding);
        }
        if self.terminal {
            return Err(E::Terminal);
        }
        if self.pending.is_some() {
            return Err(E::Pressure);
        }
        if request_id != RequestId(self.next) {
            return Err(E::Stale);
        }
        let request = self.decoder.decode(bytes).map_err(E::Request)?;
        let next = self.next.checked_add(1).ok_or(E::Terminal)?;
        let [size, count, lba, ordinal] = &mut self.numbers;
        let size = size
            .leaf(&u64::from(request.block_size).to_le_bytes())
            .map_err(E::Canonical)?;
        let count = count
            .leaf(&u64::from(request.count).to_le_bytes())
            .map_err(E::Canonical)?;
        let lba = lba
            .leaf(&u64::from(request.lba).to_le_bytes())
            .map_err(E::Canonical)?;
        let ordinal = ordinal
            .leaf(&u64::from(self.next).to_le_bytes())
            .map_err(E::Canonical)?;
        let permit = self
            .permit
            .record(&[
                validate_canonical_structured_value(size).map_err(E::Canonical)?,
                validate_canonical_structured_value(count).map_err(E::Canonical)?,
                validate_canonical_structured_value(lba).map_err(E::Canonical)?,
                validate_canonical_structured_value(ordinal).map_err(E::Canonical)?,
            ])
            .map_err(E::Canonical)?;
        let lease = self
            .authorize
            .table
            .authorize(&self.authorize.handle, &self.authorize.claim)
            .map_err(E::Capability)?;
        self.pending = Some((lease.clone(), request, self.next));
        let token = StorageAuthorizationSubmission {
            lease,
            request,
            ordinal: self.next,
            preparation: self.preparation,
        };
        self.next = next;
        Ok((token, permit))
    }
    pub(crate) fn request_cancel(&mut self) -> Result<(), StorageTransactionRefusal> {
        self.terminal = true;
        let first = self.authorize.table.revoke(&self.authorize.handle);
        let second = self.settle.table.revoke(&self.settle.handle);
        first
            .and(second)
            .map_err(StorageTransactionRefusal::Capability)
    }
    pub(crate) fn retire_cancelled(
        &mut self,
        receipt: StorageCancellationReceipt,
    ) -> Result<(), (StorageTransactionRefusal, StorageCancellationReceipt)> {
        let Some((lease, request, ordinal)) = &self.pending else {
            return Err((StorageTransactionRefusal::Stale, receipt));
        };
        if !self.terminal
            || lease != &receipt.0.lease
            || request != &receipt.0.request
            || ordinal != &receipt.0.ordinal
            || self.preparation != receipt.0.preparation
        {
            return Err((StorageTransactionRefusal::Stale, receipt));
        }
        self.pending = None;
        Ok(())
    }
    /// Candidate has exact reviewed semantic shape; class correctness remains
    /// Source-owned. Only genuine subordinate receipt allows lease retirement.
    pub(crate) fn settle(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        candidate: &[u8],
        receipt: StorageSettlementReceipt,
    ) -> Result<(), (StorageTransactionRefusal, StorageSettlementReceipt)> {
        let validate = (|| {
            use StorageTransactionRefusal as E;
            if node != self.settle.node
                || call != HostCallId(0)
                || request_id != RequestId(self.next_settle_request)
            {
                return Err(E::Binding);
            }
            if self.terminal {
                return Err(E::Terminal);
            }
            let Some((lease, request, ordinal)) = &self.pending else {
                return Err(E::Stale);
            };
            if lease != &receipt.0.lease
                || request != &receipt.0.request
                || ordinal != &receipt.0.ordinal
                || receipt.0.preparation != self.preparation
            {
                return Err(E::Stale);
            }
            self.validator.validate(candidate).map_err(E::Canonical)?;
            validate_candidate(candidate, *request, *ordinal)?;
            if self.authorize.table.inspections().any(|entry| {
                entry.lifecycle != CapabilityLifecycle::Issued || entry.in_flight_operations != 1
            }) {
                return Err(E::Terminal);
            }
            self.settle
                .table
                .authorize(&self.settle.handle, &self.settle.claim)
                .map_err(E::Capability)
        })();
        let lease = match validate {
            Ok(lease) => lease,
            Err(error) => return Err((error, receipt)),
        };
        if let Err(error) =
            self.settle
                .table
                .complete(&mut self.settle.handle, lease, candidate.len() as u32)
        {
            self.terminal = true;
            return Err((StorageTransactionRefusal::Capability(error), receipt));
        }
        if let Err(error) = self.authorize.table.complete(
            &mut self.authorize.handle,
            receipt.0.lease.clone(),
            candidate.len() as u32,
        ) {
            self.terminal = true;
            return Err((StorageTransactionRefusal::Capability(error), receipt));
        }
        self.pending = None;
        self.next_settle_request = self
            .next_settle_request
            .checked_add(1)
            .expect("bounded issued operation count");
        let value = validate_canonical_structured_value(candidate).expect("validated candidate");
        self.terminal = !matches!(
            value.variant_tag().expect("validated variant"),
            "completed" | "incomplete"
        );
        Ok(())
    }
}
fn bind_scope(
    admission: StorageScopeAdmission<'_>,
    kind: &Kind,
    call: &str,
) -> Result<Scope, StorageTransactionRefusal> {
    let node = bind_selected_operation(
        &admission.table,
        &admission.claim,
        SelectedOperationContract {
            kind,
            call,
            input_bytes: BLOCK_READ_MAXIMUM_BYTES,
            output_bytes: BLOCK_READ_MAXIMUM_BYTES,
            resource_bytes: BLOCK_READ_RESOURCE_BYTES,
        },
        admission.selected,
    )
    .map_err(|_| StorageTransactionRefusal::Binding)?;
    Ok(Scope {
        table: admission.table,
        handle: admission.handle,
        claim: admission.claim,
        node,
    })
}
fn validate_candidate(
    bytes: &[u8],
    request: BlockReadRequest,
    ordinal: u32,
) -> Result<(), StorageTransactionRefusal> {
    use StorageTransactionRefusal as E;
    let value = validate_canonical_structured_value(bytes).map_err(E::Canonical)?;
    let tag = value.variant_tag().map_err(E::Canonical)?;
    let payload = value
        .variant_payload(tag)
        .map_err(E::Canonical)?
        .ok_or(E::Candidate)?;
    let integer = |name| -> Result<u64, E> {
        let bytes = payload
            .record_field(name)
            .map_err(E::Canonical)?
            .ok_or(E::Candidate)?
            .primitive_bytes("value/u64")
            .map_err(E::Canonical)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().map_err(|_| E::Candidate)?,
        ))
    };
    if integer("block_size")? != u64::from(request.block_size)
        || integer("count")? != u64::from(request.count)
        || integer("lba")? != u64::from(request.lba)
        || integer("ordinal")? != u64::from(ordinal)
    {
        return Err(E::Candidate);
    }
    if matches!(tag, "completed" | "incomplete") {
        let actual = integer("actual")?;
        let residue = integer("residue")?;
        let data = payload
            .record_field("bytes")
            .map_err(E::Canonical)?
            .ok_or(E::Candidate)?
            .primitive_bytes("value/bytes")
            .map_err(E::Canonical)?;
        let short = payload
            .record_field("short")
            .map_err(E::Canonical)?
            .ok_or(E::Candidate)?
            .primitive_bytes("value/bool")
            .map_err(E::Canonical)?;
        if actual != data.len() as u64
            || actual > 512
            || residue != 512 - actual
            || short != [u8::from(actual < 512)]
            || (tag == "completed") != (actual == 512)
        {
            return Err(E::Candidate);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
