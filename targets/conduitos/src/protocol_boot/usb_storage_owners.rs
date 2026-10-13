//! Independent read-only storage capacity and two genuine Root-held issuers.
use super::super::{
    authority::{OperationAuthority, authority},
    usb_request::UsbStorageRequest,
};
use crate::storage_base::{contract::*, transaction_contract::*, transaction_factory::*};
use crate::{
    arch,
    cryptographic_entropy::CryptographicEntropyBase,
    protocol_source::{NativeProtocolIssuer, NativeStoragePreparation},
};
use alloc::format;
use conduit_core::*;

pub(super) const RETAINED_BYTES: u64 = 128 * 1024;
fn backing(capacity: usize, live: usize) -> Result<u64, &'static str> {
    // USB envelopes are admitted before their heap allocations occur. Reserve
    // both budgets against this same arena snapshot rather than sharing bytes.
    backing_pairs(capacity, live, 1)
}
fn backing_pairs(capacity: usize, live: usize, pairs: u64) -> Result<u64, &'static str> {
    if !(1..=2).contains(&pairs) {
        return Err("protocol-storage-arena-envelope-refused");
    }
    let required = RETAINED_BYTES
        .checked_mul(pairs)
        .ok_or("protocol-storage-arena-envelope-refused")?
        .checked_add(super::duplex_resources::HEAP_BYTES)
        .ok_or("protocol-storage-arena-envelope-refused")?;
    if capacity
        .checked_sub(live)
        .is_none_or(|bytes| (bytes as u64) < required)
    {
        return Err("protocol-storage-arena-envelope-refused");
    }
    Ok(RETAINED_BYTES)
}
/// # Safety
/// Sole trusted Root independently approved read-only block use and retains the
/// exclusively owned Boot arena. USB enumeration supplies neither approval nor
/// this capacity. Every retained allocation must pass the storage preparation guard.
pub(super) unsafe fn admit(
    request: &UsbStorageRequest,
    host: &HostId,
    boot: &BootId,
    artifact: &str,
) -> Result<NativeStoragePreparation, &'static str> {
    if request.validate().is_err() {
        return Err("protocol-storage-approval-refused");
    }
    let arena = &crate::allocation::BOOT_ARENA;
    backing_pairs(
        arena.capacity(),
        arena.live_bytes(),
        if request.read_only_reentry_approval {
            2
        } else {
            1
        },
    )?;
    let additional = if request.read_only_reentry_approval {
        Some(alloc::boxed::Box::new(unsafe {
            admit_pair(host, boot, artifact, 1, true)
        }?))
    } else {
        None
    };
    let mut primary =
        unsafe { admit_pair(host, boot, artifact, request.maximum_operations, false) }?;
    primary.additional = additional;
    if let Some(bindings) = &request.placement_bindings {
        primary.selected_gears = Some((
            bindings.authorize.as_str().into(),
            bindings.settle.as_str().into(),
        ));
        let additional = primary
            .additional
            .as_mut()
            .ok_or("protocol-storage-approval-refused")?;
        additional.selected_gears = Some((
            bindings.reentry_authorize.as_str().into(),
            bindings.reentry_settle.as_str().into(),
        ));
    }
    Ok(primary)
}

/// # Safety
/// Caller retains the independently approved arena and exact read-only scope.
unsafe fn admit_pair(
    host: &HostId,
    boot: &BootId,
    artifact: &str,
    operations: u32,
    reentry: bool,
) -> Result<NativeStoragePreparation, &'static str> {
    let maximum_retained_bytes = RETAINED_BYTES;
    let prefix = format!(
        "conduitos/protocol/{}/read-only-storage{}",
        boot.as_str(),
        if reentry { "/reentry" } else { "" }
    );
    let authorize_capability = if reentry {
        format!("{AUTHORIZE_IMPLEMENTATION}/reentry")
    } else {
        AUTHORIZE_IMPLEMENTATION.into()
    };
    let settle_capability = if reentry {
        format!("{SETTLE_IMPLEMENTATION}/reentry")
    } else {
        SETTLE_IMPLEMENTATION.into()
    };
    let provider = BaseInstanceId::from(format!("{prefix}/provider"));
    let pool = ResourcePoolId::from(format!("{prefix}/retention"));
    let generation = ResourceGenerationId(format!("{prefix}/generation/1"));
    let envelope = CapabilityEnvelopeId::from(format!("{prefix}/envelope"));
    let source = arch::DomainEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
    let mut entropy =
        CryptographicEntropyBase::<_, 2>::admit(source).map_err(|_| "protocol-entropy-refused")?;
    let mut issue = |name, call, kind, implementation| {
        let scope = authority(
            host,
            boot,
            name,
            (&provider, &pool, &generation, &envelope),
            OperationAuthority {
                contract: BLOCK_READ_AUTHORITY,
                call,
                kind,
                capability: implementation,
                bytes: BLOCK_READ_MAXIMUM_BYTES,
                work: 1,
                operations,
            },
        );
        // SAFETY: each issuer receives its independently approved storage scope and fresh key.
        unsafe { NativeProtocolIssuer::admit(scope, &mut entropy) }
            .map_err(|_| "protocol-storage-issuance-refused")
    };
    let authorize = issue(
        if reentry {
            "block-read-authorize-reentry"
        } else {
            "block-read-authorize"
        },
        AUTHORIZE_CALL,
        AUTHORIZE_KIND,
        authorize_capability.as_str(),
    )?;
    let settle = issue(
        if reentry {
            "block-read-settle-reentry"
        } else {
            "block-read-settle"
        },
        SETTLE_CALL,
        SETTLE_KIND,
        settle_capability.as_str(),
    )?;
    Ok(NativeStoragePreparation {
        host: host.clone(),
        boot: boot.clone(),
        provider,
        pool,
        artifact: artifact.into(),
        maximum_retained_bytes,
        authorize,
        settle,
        additional: None,
        selected_gears: None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reentry_requires_both_independent_storage_envelopes() {
        let required = (2 * RETAINED_BYTES + super::super::duplex_resources::HEAP_BYTES) as usize;
        assert_eq!(backing_pairs(required + 32, 32, 2), Ok(RETAINED_BYTES));
        assert!(backing_pairs(required + 31, 32, 2).is_err());
        assert!(backing_pairs(required, 0, 0).is_err());
        assert!(backing_pairs(required, 0, 3).is_err());
    }
    #[test]
    fn actual_arena_headroom_backs_storage_separately_from_usb() {
        let required = (RETAINED_BYTES + super::super::duplex_resources::HEAP_BYTES) as usize;
        assert_eq!(backing(required + 32, 32), Ok(RETAINED_BYTES));
        assert!(backing(required + 31, 32).is_err());
        assert!(backing(RETAINED_BYTES as usize, 0).is_err());
        assert!(backing(usize::MAX, usize::MAX - required + 1).is_err());
        assert_eq!(
            backing(usize::MAX, usize::MAX - required),
            Ok(RETAINED_BYTES)
        );
        assert!(backing(0, 0).is_err());
        assert!(backing(32, 33).is_err());
    }
}
