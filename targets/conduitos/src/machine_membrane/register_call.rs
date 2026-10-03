//! Exact primitive ABI for register access over the admitted Host Call boundary.
//!
//! The request is one U128: offset in bits 0..31, write value in 32..63,
//! operation (0=read, 1=write) in 64..71; all other bits are reserved zero.
//! Read requests require a zero value. No request contains a mapping/address,
//! resource selector, authority ID or constructible capability.

use alloc::vec;
use conduit_core::{
    CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal, kind_id,
    port_id,
};
use conduit_kernel::{Failure, FailureCode, HostCallId, NodeId};

use super::{REGISTER_KIND, RegisterLeaf, RegisterOperation, RegisterRefusal};

pub const REGISTER_REQUEST_BYTES: u32 = 16;
pub const REGISTER_RESULT_BYTES: u32 = 4;

/// Preparation-only semantic contract. Implementations, resources and authority
/// remain separate planning/binding facts, absent from this Kind.
pub fn contract() -> Kind {
    let port = |name, value, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(REGISTER_KIND),
        kind_contract_revision: KindIdentity::from("machine/memory/mmio/register32@2"),
        startup_parameters: vec![],
        shorthand: Some((port_id("request"), port_id("value"))),
        inputs: vec![port("request", "value/u128", PortDirection::Input)],
        outputs: vec![port("value", "value/u32", PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: REGISTER_REQUEST_BYTES,
        },
    }
}

/// Definition catalogs for checking/expansion, independent of availability.
/// A host may advertise a Back only after its native resource is safely bound.
pub fn catalogs() -> (conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog) {
    let kind = contract();
    let mut startup = conduit_plot::StartupCatalog::new();
    startup
        .insert(conduit_plot::KindSignature {
            kind: REGISTER_KIND.into(),
            startup_parameters: vec![],
        })
        .expect("unique register startup definition");
    startup
        .insert_fore(REGISTER_KIND, kind.checked_front())
        .expect("exact register Fore");
    let mut profile = conduit_plot::ProfileCatalog::new();
    profile
        .insert_kind(kind)
        .expect("unique register Kind definition");
    (startup, profile)
}

/// One selected leaf bound to one exact lowered node/call slot. There is no
/// lookup by descriptive resource identity and no fallback to another leaf.
pub struct RegisterHostCall {
    pub(super) node: NodeId,
    pub(super) leaf: RegisterLeaf,
}

impl RegisterHostCall {
    /// The native composition root supplies the node selected for this leaf's
    /// exact Plan/Play possession, before installing the sealed call table.
    #[cfg(test)]
    pub(crate) fn new(node: NodeId, leaf: RegisterLeaf) -> Self {
        Self { node, leaf }
    }

    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        input: &[u8],
    ) -> Result<[u8; 4], RegisterRefusal> {
        self.check_binding(node, call)?;
        let request: &[u8; 16] = input
            .try_into()
            .map_err(|_| RegisterRefusal::InvalidRequest)?;
        if request[9..].iter().any(|byte| *byte != 0) {
            return Err(RegisterRefusal::InvalidRequest);
        }
        let offset = u32::from_le_bytes(request[..4].try_into().expect("fixed offset"));
        let value = u32::from_le_bytes(request[4..8].try_into().expect("fixed value"));
        let operation = match request[8] {
            0 if value == 0 => RegisterOperation::Read32 { offset },
            1 => RegisterOperation::Write32 { offset, value },
            _ => return Err(RegisterRefusal::InvalidRequest),
        };
        self.leaf.invoke(operation).map(u32::to_le_bytes)
    }

    /// Synchronous MMIO has no outstanding DMA. Revocation prevents subsequent
    /// dispatch, and the kernel rejects a completion after cancellation.
    pub fn cancel(&mut self, node: NodeId, call: HostCallId) -> Result<(), RegisterRefusal> {
        self.check_binding(node, call)?;
        self.leaf.revoke()
    }

    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), RegisterRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(RegisterRefusal::WrongBinding);
        }
        Ok(())
    }
}

impl RegisterRefusal {
    /// Stable kernel failure details keep malformed data, authority, exhausted
    /// work and unsupported machine behavior distinct in retained signs.
    pub fn failure(&self) -> Failure {
        use conduit_core::BaseCapabilityRefusal as Base;
        let (code, detail) = match self {
            Self::InvalidRequest => (FailureCode::InvalidInput, 1),
            Self::Alignment => (FailureCode::InvalidInput, 2),
            Self::Range => (FailureCode::InvalidInput, 3),
            Self::ReadOnly => (FailureCode::HostCallDenied, 4),
            Self::Possession => (FailureCode::HostCallDenied, 5),
            Self::UnsupportedOrdering => (FailureCode::HostCallFailed, 6),
            Self::WrongBinding => (FailureCode::HostCallDenied, 7),
            Self::Capability(reason) => {
                let detail = match reason {
                    Base::InvalidIssuer => 101,
                    Base::TableFull => 102,
                    Base::EmptyIdentity => 103,
                    Base::InvalidBound => 104,
                    Base::AuthorityMismatch => 105,
                    Base::ScopeBroadening => 106,
                    Base::UnknownCapability => 107,
                    Base::Revoked => 108,
                    Base::Exhausted => 109,
                    Base::WrongScope => 110,
                    Base::ParameterEnvelope => 111,
                    Base::WorkEnvelope => 112,
                    Base::InFlightFull => 113,
                    Base::UnknownLease => 114,
                    Base::StaleCompletion => 115,
                    Base::ResultEnvelope => 116,
                };
                let code = if matches!(reason, Base::Exhausted | Base::WorkEnvelope) {
                    FailureCode::WorkBudgetExhausted
                } else {
                    FailureCode::HostCallDenied
                };
                (code, detail)
            }
        };
        Failure { code, detail }
    }
}
