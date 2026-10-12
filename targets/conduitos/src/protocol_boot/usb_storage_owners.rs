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
    let required = RETAINED_BYTES
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
    if !request.administrator_approval || !(1..=16).contains(&request.maximum_operations) {
        return Err("protocol-storage-approval-refused");
    }
    let arena = &crate::allocation::BOOT_ARENA;
    let maximum_retained_bytes = backing(arena.capacity(), arena.live_bytes())?;
    let prefix = format!("conduitos/protocol/{}/read-only-storage", boot.as_str());
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
                operations: request.maximum_operations,
            },
        );
        // SAFETY: each issuer receives its independently approved storage scope and fresh key.
        unsafe { NativeProtocolIssuer::admit(scope, &mut entropy) }
            .map_err(|_| "protocol-storage-issuance-refused")
    };
    let authorize = issue(
        "block-read-authorize",
        AUTHORIZE_CALL,
        AUTHORIZE_KIND,
        AUTHORIZE_IMPLEMENTATION,
    )?;
    let settle = issue(
        "block-read-settle",
        SETTLE_CALL,
        SETTLE_KIND,
        SETTLE_IMPLEMENTATION,
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
    })
}
#[cfg(test)]
mod tests {
    use super::*;
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
