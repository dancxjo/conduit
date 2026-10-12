//! Containing-machine memory admission, separate from the one queue ownership unit.
//! Root's fixed Make arena backs this finite budget; USB retains its own pending
//! heap envelopes. Neither this geometry nor queue discovery grants authority.
use super::{
    installation::EthernetFrameNativeRefusal, owned_installation::ReadyOwnedEthernetFrameBase,
};
use conduit_core::{BaseCapabilityTable, BaseOperationClaim};

pub(crate) const OWNED_RETENTION_BYTES: usize = 128 * 1024;
const METADATA_BYTES: usize = 16 * 1024;

pub(crate) fn rounded(bytes: usize) -> Option<usize> {
    bytes.checked_add(31).map(|value| value & !31)
}

/// Both budgets must have actual remaining backing before Root allocates the
/// network container. This does not reuse either USB endpoint's byte envelope.
pub(crate) fn admit_backing(
    capacity: usize,
    live: usize,
    pending_usb_heap: usize,
) -> Result<(), EthernetFrameNativeRefusal> {
    let needed = OWNED_RETENTION_BYTES.checked_add(pending_usb_heap);
    if needed.is_none_or(|needed| capacity.checked_sub(live).is_none_or(|free| free < needed)) {
        return Err(EthernetFrameNativeRefusal::Binding);
    }
    Ok(())
}

pub(crate) struct NetworkRetention {
    existing: usize,
    before: usize,
}
impl NetworkRetention {
    pub(crate) fn begin(
        ready: &ReadyOwnedEthernetFrameBase,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
    ) -> Result<Self, EthernetFrameNativeRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(EthernetFrameNativeRefusal::Binding)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || table.maximum_capabilities() != 1
            || table.reserved_capability_slots() != 1
        {
            return Err(EthernetFrameNativeRefusal::Binding);
        }
        let metadata = table
            .retained_storage_bytes(32)
            .and_then(|bytes| bytes.checked_add(claim.retained_storage_bytes(32)?))
            .filter(|bytes| *bytes <= METADATA_BYTES)
            .ok_or(EthernetFrameNativeRefusal::Binding)?;
        let existing = ready
            .retained_preparation_bytes()
            .and_then(|bytes| bytes.checked_add(metadata))
            .ok_or(EthernetFrameNativeRefusal::Binding)?;
        charge(existing, 0, 0)?;
        Ok(Self {
            existing,
            before: crate::allocation::BOOT_ARENA.live_bytes(),
        })
    }

    /// Called after binding, socket preparation, boxing the host and appending
    /// dispatcher identities, so their heap growth is inside this measurement.
    pub(crate) fn finish(self) -> Result<(), EthernetFrameNativeRefusal> {
        let arena = &crate::allocation::BOOT_ARENA;
        if arena.capacity() == 0 {
            // Hosted conformance uses its allocator; native admission requires
            // the actual containing-machine arena and its measured receipt.
            return Ok(());
        }
        let retained = charge(self.existing, self.before, arena.live_bytes())?;
        use core::fmt::Write;
        let mut line = crate::sign_format::FixedText::new();
        writeln!(
            &mut line,
            "CONDUIT_USB_NETWORK_PREPARE retained={} admitted={} preexisting={} before={} after={}",
            retained,
            OWNED_RETENTION_BYTES,
            self.existing,
            self.before,
            arena.live_bytes()
        )
        .map_err(|_| EthernetFrameNativeRefusal::Binding)?;
        crate::arch::append_boot_diagnostic(line.as_bytes())
            .map_err(|_| EthernetFrameNativeRefusal::Binding)
    }
}

fn charge(
    existing: usize,
    before: usize,
    after: usize,
) -> Result<usize, EthernetFrameNativeRefusal> {
    // Subtract released preparation metadata only after charging its actual
    // initial allocation. Saturating the delta would hide released capacities.
    existing
        .checked_add(after)
        .and_then(|total| total.checked_sub(before))
        .filter(|total| *total <= OWNED_RETENTION_BYTES)
        .ok_or(EthernetFrameNativeRefusal::Binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_and_unconsumed_usb_budgets_need_disjoint_actual_backing() {
        let usb = 152 * 1024;
        let live = 4096;
        assert!(admit_backing(live + usb + OWNED_RETENTION_BYTES, live, usb).is_ok());
        assert!(admit_backing(live + usb + OWNED_RETENTION_BYTES - 1, live, usb).is_err());
        assert!(admit_backing(live + OWNED_RETENTION_BYTES, live, usb).is_err());
        assert!(admit_backing(0, 0, 0).is_err());
        assert!(admit_backing(usize::MAX, 0, usize::MAX).is_err());
    }
    #[test]
    fn charge_includes_preallocated_storage_and_retained_dispatcher_growth() {
        let existing = 16384;
        let before = 1_000_000;
        let after = before + OWNED_RETENTION_BYTES - existing;
        assert_eq!(
            charge(existing, before, after).unwrap(),
            OWNED_RETENTION_BYTES
        );
        assert!(charge(existing, before, after + 1).is_err());
        assert!(charge(existing, 0, OWNED_RETENTION_BYTES).is_err());
        assert_eq!(
            charge(existing, before, before - 4096).unwrap(),
            existing - 4096
        );
        assert!(charge(existing, before, before - existing - 1).is_err());
        assert!(charge(usize::MAX, 0, 1).is_err());
    }
    #[test]
    fn allocator_rounding_and_hidden_capacity_are_charged() {
        assert_eq!(rounded(1), Some(32));
        assert_eq!(rounded(33), Some(64));
        assert_eq!(rounded(usize::MAX), None);
        let mut short = alloc::string::String::from("short");
        short.reserve(OWNED_RETENTION_BYTES);
        assert!(charge(rounded(short.capacity()).unwrap(), 0, 0).is_err());
    }
}
