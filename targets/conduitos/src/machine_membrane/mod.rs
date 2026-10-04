//! Capability-bound register leaves. Protocol policy belongs in checked plots.
//!
//! This is the cooperative provider seam. A compliant call validates possession;
//! shared-address-space execution is not hostile-code confinement.

use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
};
use core::ptr::{read_volatile, write_volatile};

mod ordering;
mod register_binding;
pub mod register_call;
pub(crate) mod selected_operation;
#[cfg(test)]
pub(crate) mod selection_fixture;

pub const MAX_REGISTER_WINDOW_BYTES: u32 = 65536;
pub const REGISTER_KIND: &str = "machine/memory/mmio/register32";
pub const REGISTER_CALL: &str = "conduit.host/machine-register32@2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterOperation {
    Read32 { offset: u32 },
    Write32 { offset: u32, value: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegisterRefusal {
    Possession,
    InvalidRequest,
    WrongBinding,
    Alignment,
    Range,
    ReadOnly,
    UnsupportedOrdering,
    Capability(BaseCapabilityRefusal),
}

/// Trusted immutable envelope for one mapped register window. No address is
/// accepted in a runtime request; offset bounds are independently enforced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterEnvelope {
    pub bytes: u32,
    pub writable: bool,
}

pub struct RegisterLeaf {
    table: BaseCapabilityTable,
    claim: BaseOperationClaim,
    handle: BaseCapabilityHandle,
    mapped: *mut u32,
    envelope: RegisterEnvelope,
}

impl RegisterLeaf {
    /// Bind an already admitted mapping to current possession. This constructor
    /// belongs to the trusted native composition root, never to a plot or a
    /// request decoder. IDs and offsets alone cannot construct a leaf.
    ///
    /// # Safety
    /// `mapped` must be a suitably aligned, device-memory mapped region valid
    /// for `envelope.bytes` for this possession's lifetime. The caller owns its
    /// mapping/resource lifecycle and must revoke before unmapping. Each
    /// register in the envelope must permit aligned 32-bit access. The mapping
    /// and envelope must be the exact resource/generation and permissions
    /// already authorized by the claim; this constructor cannot mint mapping
    /// authority or broaden the trusted authority ceiling. Permitted writes
    /// must not bypass mandatory safety or program access outside that ceiling:
    /// a DMA-controlling window requires independently enforced confinement to
    /// admitted DMA storage. A scoped window/Plan alone does not provide that
    /// confinement. USB class plots use a bounded endpoint transfer base; they
    /// must not receive arbitrary controller-register programming authority.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) unsafe fn from_admitted_mapping(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        mapped: *mut u32,
        envelope: RegisterEnvelope,
    ) -> Result<Self, RegisterRefusal> {
        if mapped.is_null() || mapped as usize & 3 != 0 || envelope.bytes == 0 {
            return Err(RegisterRefusal::Alignment);
        }
        if envelope.bytes > MAX_REGISTER_WINDOW_BYTES
            || envelope.bytes & 3 != 0
            || (mapped as usize)
                .checked_add(envelope.bytes as usize)
                .is_none()
        {
            return Err(RegisterRefusal::Range);
        }
        // This leaf owns one possession and one exact operation claim. Check
        // the fixed completion bound before any native access can occur.
        {
            let mut entries = table.inspections();
            let entry = entries.next().ok_or(RegisterRefusal::Possession)?;
            if entries.next().is_some()
                || entry.scope.maximum_result_bytes < 4
                || claim.operation_contract_id.as_str() != REGISTER_CALL
                || claim.subject_kind.as_str() != REGISTER_KIND
                || claim.parameter_bytes != register_call::REGISTER_REQUEST_BYTES
                || claim.work_units != 1
            {
                return Err(RegisterRefusal::Possession);
            }
        }
        Ok(Self {
            table,
            claim,
            handle,
            mapped,
            envelope,
        })
    }

    pub fn invoke(&mut self, operation: RegisterOperation) -> Result<u32, RegisterRefusal> {
        let offset = match operation {
            RegisterOperation::Read32 { offset } | RegisterOperation::Write32 { offset, .. } => {
                offset
            }
        };
        if offset & 3 != 0 {
            return Err(RegisterRefusal::Alignment);
        }
        if offset
            .checked_add(4)
            .is_none_or(|end| end > self.envelope.bytes)
        {
            return Err(RegisterRefusal::Range);
        }
        if matches!(operation, RegisterOperation::Write32 { .. }) && !self.envelope.writable {
            return Err(RegisterRefusal::ReadOnly);
        }
        ordering::supported()?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(RegisterRefusal::Capability)?;
        ordering::before();
        let register = unsafe { self.mapped.cast::<u8>().add(offset as usize).cast::<u32>() };
        let value = match operation {
            RegisterOperation::Read32 { .. } => u32::from_le(unsafe { read_volatile(register) }),
            RegisterOperation::Write32 { value, .. } => {
                unsafe { write_volatile(register, value.to_le()) };
                value
            }
        };
        ordering::after();
        self.table
            .complete(&mut self.handle, lease, 4)
            .map_err(RegisterRefusal::Capability)?;
        Ok(value)
    }

    /// Called by the resource owner before cancellation/replacement unmaps the
    /// native region. Revocation does not invent a successful device stop.
    pub fn revoke(&mut self) -> Result<(), RegisterRefusal> {
        self.table
            .revoke(&self.handle)
            .map_err(RegisterRefusal::Capability)
    }
}

#[cfg(test)]
mod tests;
