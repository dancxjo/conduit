//! Exact planned possession for an asynchronous electrical pulse Host Call.
use crate::*;
use conduit_plan_lowering::selected_assigned_operation::{
    bind_selected_assigned_operation, AssignedNativeIdentity, SelectedAssignedOperationPlan,
};
pub struct AssignedPulseSelection<'a> {
    pub contract: &'a PulseReadContract,
    pub native: AssignedNativeIdentity<'a>,
    pub plan: SelectedAssignedOperationPlan<'a>,
}

use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
    BaseOperationLease, CapabilityInspection, StructuredInfoRefusal,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::selected_operation::{
    bind_selected_operation, SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan,
};

/// A native provider must terminate within its admitted deadline and quiesce
/// electrical work when its returned future is dropped. It must not allocate in
/// Play. Ownership and attachment limits come from native installation, not Source.
/// `Err` denotes refused geometry before effects; physical failures must return
/// a capture with an explicit timeout or provider-failed disposition.
pub trait PulseReadProvider {
    fn read(
        &mut self,
        request: CheckedPulseRead,
    ) -> impl core::future::Future<Output = Result<PulseReadCapture, PulseReadRefusal>>;
}

/// Independently authorized native attachment truth.
#[derive(Clone, Copy, Debug)]
pub struct PulseAttachment {
    pub generation: u64,
    pub resource_bytes: u64,
    pub limits: PulseReadLimits,
}

impl PulseAttachment {
    pub(crate) fn valid(self) -> bool {
        self.generation != 0
            && self.resource_bytes >= MAXIMUM_OCTETS as u64
            && CheckedPulseRead::admit(
                PulseReadRequest {
                    hold_low_us: 1,
                    skip_high_pulses: 0,
                    sample_after_rise_us: self.limits.minimum_sample_offset_us,
                    octets: 1,
                    capture_timeout_us: self.limits.maximum_capture_timeout_us,
                },
                self.limits,
            )
            .is_ok()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PulseCallRefusal {
    WrongBinding,
    Possession,
    StaleRequest,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Codec(PulseCodecRefusal),
    Canonical(StructuredInfoRefusal),
}

pub struct PulseHostCall<P> {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    attachment: PulseAttachment,
    provider: P,
    node: NodeId,
    next_request: u32,
    codec: PreparedPulseCodec,
}
impl<P: PulseReadProvider> PulseHostCall<P> {
    /// Bind actual native ownership to an already admitted exact Plan and Play.
    ///
    /// # Safety
    /// The trusted composition root must own the exact electrical resources and
    /// attachment generation under this claim. The provider must enforce finite
    /// termination and cancellation quiescence. This cooperative boundary does
    /// not confine hostile native code sharing the address space.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: PulseAttachment,
        provider: P,
        contract: &PulseReadContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, PulseCallRefusal> {
        check_possession(&table, &claim, attachment)?;
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: PULSE_READ_CALL,
                input_bytes: PULSE_READ_MAXIMUM_BYTES,
                output_bytes: PULSE_READ_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            selection,
        )
        .map_err(|reason| match reason {
            SelectedCallRefusal::WrongBinding => PulseCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => PulseCallRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            attachment,
            provider,
            node,
            next_request: 0,
            codec: PreparedPulseCodec::prepare(contract).map_err(PulseCallRefusal::Canonical)?,
        })
    }

    /// Bind an explicitly admitted constrained projection to real possession.
    /// # Safety
    /// The native Root must own these exact electrical resources, independently
    /// authorize this Plan/Play and retain the provider's finite/cancellation laws.
    /// The native identity and resident inventories must not come from the input.
    pub unsafe fn bind_assigned(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: PulseAttachment,
        provider: P,
        selection: AssignedPulseSelection<'_>,
    ) -> Result<Self, PulseCallRefusal> {
        check_possession(&table, &claim, attachment)?;
        let node = bind_selected_assigned_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: selection.contract.kind(),
                call: PULSE_READ_CALL,
                input_bytes: PULSE_READ_MAXIMUM_BYTES,
                output_bytes: PULSE_READ_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            selection.native,
            selection.plan,
        )
        .map_err(|e| match e {
            SelectedCallRefusal::WrongBinding => PulseCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => PulseCallRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            attachment,
            provider,
            node,
            next_request: 0,
            codec: PreparedPulseCodec::prepare(selection.contract)
                .map_err(PulseCallRefusal::Canonical)?,
        })
    }

    pub async fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], PulseCallRefusal> {
        self.check_binding(node, call)?;
        if request_id != RequestId(self.next_request) {
            return Err(PulseCallRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(PulseCallRefusal::SequenceExhausted)?;
        let geometry = match self.codec.request(input, self.attachment.limits) {
            Ok(request) => Some(request),
            Err(PulseCodecRefusal::Geometry(_)) => None,
            Err(reason) => return Err(PulseCallRefusal::Codec(reason)),
        };
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(PulseCallRefusal::Capability)?;
        self.next_request = next;
        // This guard is created before the provider future: cancellation first
        // drops/quiesces electrical work, then revokes the unfinished capability.
        let lease = PendingLease {
            table: &mut self.table,
            handle: &mut self.handle,
            lease: Some(lease),
        };
        let encoded = match geometry {
            Some(request) => match self.provider.read(request).await {
                Ok(capture) => self.codec.result(capture),
                Err(_) => self.codec.geometry_refused(),
            },
            None => self.codec.geometry_refused(),
        }
        .map_err(PulseCallRefusal::Codec);
        let bytes = encoded.as_ref().map_or(0, |bytes| bytes.len() as u32);
        lease
            .complete(bytes)
            .map_err(PulseCallRefusal::Capability)?;
        encoded
    }

    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), PulseCallRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(PulseCallRefusal::Capability)
    }

    pub fn inspection(&self) -> &CapabilityInspection {
        self.table
            .inspections()
            .next()
            .expect("single prepared capability")
    }

    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), PulseCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(PulseCallRefusal::WrongBinding);
        }
        Ok(())
    }
}

struct PendingLease<'a> {
    table: &'a mut BaseCapabilityTable,
    handle: &'a mut BaseCapabilityHandle,
    lease: Option<BaseOperationLease>,
}
impl PendingLease<'_> {
    fn complete(mut self, result_bytes: u32) -> Result<(), BaseCapabilityRefusal> {
        let lease = self.lease.as_ref().expect("pending").clone();
        self.table.complete(self.handle, lease, result_bytes)?;
        self.lease = None;
        Ok(())
    }
}
impl Drop for PendingLease<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            // A cancelled dispatch cannot be resumed or replayed under this grant.
            let _ = self.table.revoke(self.handle);
        }
    }
}

fn check_possession(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    attachment: PulseAttachment,
) -> Result<(), PulseCallRefusal> {
    let mut entries = table.inspections();
    let entry = entries.next().ok_or(PulseCallRefusal::Possession)?;
    if entries.next().is_some()
        || entry.scope.maximum_in_flight != 1
        || entry.scope.maximum_parameter_bytes < PULSE_READ_MAXIMUM_BYTES
        || entry.scope.maximum_result_bytes < PULSE_READ_MAXIMUM_BYTES
        || claim.parameter_bytes != PULSE_READ_MAXIMUM_BYTES
        || claim.work_units != 1
        || !attachment.valid()
    {
        return Err(PulseCallRefusal::Possession);
    }
    drop(entries);
    Ok(())
}
