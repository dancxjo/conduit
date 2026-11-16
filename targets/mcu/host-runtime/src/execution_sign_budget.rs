//! Explicit finite evidence for one-input/one-result duplex Line work.
//! This is a capacity law, not execution authority or a streaming archive.
use conduit_kernel::{HostedSignLog, KernelEvent, SignError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuplexSignBudget {
    ingress_items: u16,
    local_items: u16,
    remote_items: u16,
    total_items: u16,
}
impl DuplexSignBudget {
    /// Preserve every original local slot. Each accepted input reserves its
    /// admission plus output offer/accept/delivery; two additional remote slots
    /// retain input/output closure. Caller must install this ingress window and
    /// supply an actual one-result-per-input service, never infer that from IO.
    pub const fn select(local: u16, item_ceiling: u16, byte_ceiling: usize) -> Option<Self> {
        if local == 0 || item_ceiling < local {
            return None;
        }
        let extra = item_ceiling - local;
        if extra < 6 {
            return None;
        }
        let ingress = (extra - 2) / 4;
        let remote = ingress * 4 + 2;
        let total = local + remote;
        let Some(bytes) = HostedSignLog::canonical_storage_bytes(total, remote) else {
            return None;
        };
        if bytes > byte_ceiling {
            return None;
        }
        let budget = Self {
            ingress_items: ingress,
            local_items: local,
            remote_items: remote,
            total_items: total,
        };
        match budget.logical_byte_charge() {
            Some(logical) if logical <= byte_ceiling => Some(budget),
            _ => None,
        }
    }
    pub const fn ingress_items(self) -> u16 {
        self.ingress_items
    }
    pub const fn local_items(self) -> u16 {
        self.local_items
    }
    pub const fn remote_items(self) -> u16 {
        self.remote_items
    }
    pub const fn total_items(self) -> u16 {
        self.total_items
    }
    pub const fn backing_bytes(self) -> Option<usize> {
        HostedSignLog::canonical_storage_bytes(self.total_items, self.remote_items)
    }
    /// Both declared log capacities remain funded even when packed physical
    /// event storage is smaller. Do not infer target widths from a host build.
    pub const fn logical_byte_charge(self) -> Option<usize> {
        let Some(events) =
            (self.total_items as usize).checked_mul(core::mem::size_of::<KernelEvent>())
        else {
            return None;
        };
        let Some(remote) = conduit_kernel::remote_sign_storage_bytes(self.remote_items) else {
            return None;
        };
        if remote as u64 > usize::MAX as u64 {
            return None;
        }
        events.checked_add(remote as usize)
    }
    pub fn prepare(self) -> Result<HostedSignLog, SignError> {
        HostedSignLog::new_with_remote_storage(
            self.total_items,
            u32::from(self.total_items) * core::mem::size_of::<KernelEvent>() as u32,
            self.remote_items,
            conduit_kernel::remote_sign_storage_bytes(self.remote_items)
                .ok_or(SignError::InvalidBudget)?,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_window_preserves_local_and_closure_stock_within_original_ceiling() {
        let b = DuplexSignBudget::select(55, 64, 24576).unwrap();
        assert_eq!((b.ingress_items, b.remote_items, b.total_items), (1, 6, 61));
        assert_eq!(b.total_items - b.remote_items, 55);
        assert!(DuplexSignBudget::select(55, 60, 24576).is_none());
        assert!(DuplexSignBudget::select(55, 64, 1).is_none());
        assert_eq!(
            DuplexSignBudget::select(16, 26, 24576)
                .unwrap()
                .ingress_items,
            2
        );
    }
    #[test]
    fn packed_backing_cannot_enlarge_the_declared_logical_byte_grant() {
        let b = DuplexSignBudget::select(55, 64, 24576).unwrap();
        let physical = b.backing_bytes().unwrap();
        let logical = b.logical_byte_charge().unwrap();
        let required = physical.max(logical);
        assert!(DuplexSignBudget::select(55, 64, required - 1).is_none());
        assert_eq!(DuplexSignBudget::select(55, 64, required), Some(b));
        if physical < logical {
            let between = logical - 1;
            assert!(physical <= between);
            assert!(
                DuplexSignBudget::select(55, 64, between).is_none(),
                "physical fit never authorizes larger logical capacities"
            );
        }
    }
}
