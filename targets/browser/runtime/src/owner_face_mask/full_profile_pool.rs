//! Exclusive cooperative requested-storage budgets for the optional full Mask.
//! No unused buffer substitutes for actual cache/interface/model ownership.
//! The separate bootstrap allowance precedes Full interface/cache preparation;
//! two Mask slots remain exclusively available for current/replacement custody.
//! These limits are not a hostile-code allocator-confinement boundary.
use conduit_browser_mask_offer::full_profile as profile;
use conduit_core::{BootId, HostAdvertisement, HostId};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[path = "full_profile_pool/cache.rs"]
mod cache;
pub(crate) use cache::Evidence;
/// Explicit optional Host bootstrap allowance, independent of the Body runner.
/// This pays actual Full ABI buffers, canonical offer witness, generic template
/// and their preparation overlap. Compiled-template peak is qualified by tests.
pub(crate) const BOOTSTRAP_BYTES: usize = 16 * 1024 * 1024;
struct Pool {
    host: HostId,
    boot: BootId,
    frozen: bool,
    interface_bytes: usize,
    baseline_admitted: usize,
    evidence: Option<Rc<Evidence>>,
    occupied: [bool; profile::PREPARATION_SLOTS],
}
thread_local! { static POOL:RefCell<Option<Box<Pool>>> = const { RefCell::new(None) }; }
fn metadata_bytes(host: &HostId, boot: &BootId) -> usize {
    std::mem::size_of::<Pool>()
        + std::mem::size_of::<RefCell<Option<Box<Pool>>>>()
        + host.as_str().len()
        + boot.as_str().len()
}
pub(crate) fn prepare(host: &HostId, boot: &BootId) -> Result<(), String> {
    if host.as_str().is_empty()
        || boot.as_str().is_empty()
        || host.as_str().len() > 128
        || boot.as_str().len() > 128
    {
        return Err("Mask pool identity extent".into());
    }
    let metadata = metadata_bytes(host, boot);
    if metadata > BOOTSTRAP_BYTES {
        return Err("Mask bootstrap metadata pressure".into());
    }
    POOL.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_some() {
            return Err("Mask preparation pool already initialized".into());
        }
        *s = Some(Box::new(Pool {
            host: host.clone(),
            boot: boot.clone(),
            frozen: false,
            interface_bytes: 0,
            baseline_admitted: metadata,
            evidence: None,
            occupied: [false; profile::PREPARATION_SLOTS],
        }));
        Ok(())
    })
}
/// Reserve additional bootstrap work BEFORE its concrete allocations. This is
/// a high-water allowance, not a cumulative allocation-traffic counter.
pub(crate) fn admit_bootstrap(bytes: usize) -> Result<(), String> {
    POOL.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.as_mut().ok_or("Mask bootstrap pool absent")?;
        if p.frozen {
            return Err("Mask bootstrap already frozen".into());
        }
        let total = metadata_bytes(&p.host, &p.boot)
            .checked_add(bytes)
            .ok_or("Mask bootstrap extent")?;
        if total > BOOTSTRAP_BYTES {
            return Err("Mask bootstrap bytes pressure".into());
        }
        p.baseline_admitted = p.baseline_admitted.max(total);
        Ok(())
    })
}
/// Called only after the ABI owns the concrete buffers at these capacities.
/// No Full offer may be constructed before this successful capacity check.
pub(crate) fn mark_interface_ready(input: usize, output: usize) -> Result<(), String> {
    use conduit_host_browser_make as limits;
    if input != limits::OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize + 12 * 1024
        || output != limits::OWNER_FACE_FULL_MAXIMUM_VIEW_BYTES as usize
    {
        return Err("Mask Full interface capacity mismatch".into());
    }
    let actual = input
        .checked_add(output)
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<Vec<u8>>()))
        .ok_or("Mask interface extent")?;
    POOL.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.as_mut().ok_or("Mask bootstrap absent")?;
        if p.frozen || p.interface_bytes != 0 {
            return Err("Mask interface already initialized".into());
        }
        if metadata_bytes(&p.host, &p.boot) + actual > p.baseline_admitted {
            return Err("Mask interface allocation was not admitted".into());
        }
        p.interface_bytes = actual;
        Ok(())
    })
}
/// Internal advertisement construction readiness. Export still MUST complete
/// capture_advertisement and freeze; this avoids a circular template dependency.
pub(crate) fn interface_ready() -> bool {
    POOL.with(|s| s.borrow().as_ref().is_some_and(|p| p.interface_bytes != 0))
}
pub(crate) fn ready(host: &HostId, boot: &BootId) -> bool {
    POOL.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|p| &p.host == host && &p.boot == boot && p.interface_bytes != 0)
    })
}
pub(crate) fn capture_advertisement(host: &HostAdvertisement, bytes: &[u8]) -> Result<(), String> {
    POOL.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.as_mut().ok_or("Mask bootstrap absent")?;
        if p.interface_bytes == 0 || p.host != host.host_id || p.boot != host.boot_id {
            return Err("Mask snapshot incarnation or lifecycle".into());
        }
        if let Some(evidence) = &p.evidence {
            cache::verify_bytes(host, bytes)?;
            if evidence.canonical_bytes() == bytes
                && evidence.offer_generation() == host.offer_generation
            {
                return Ok(());
            }
            return Err("Mask Boot snapshot is immutable".into());
        }
        if p.frozen {
            return Err("Mask snapshot already frozen".into());
        }
        // Reserve the dedicated allowance before running the compiled planner.
        // Actual retained capacities are checked separately after preparation;
        // the planner's transient peak is an explicitly qualified Host profile.
        p.baseline_admitted = BOOTSTRAP_BYTES;
        let permanent = metadata_bytes(&p.host, &p.boot)
            .checked_add(p.interface_bytes)
            .ok_or("Mask bootstrap extent")?;
        let available = BOOTSTRAP_BYTES
            .checked_sub(permanent)
            .ok_or("Mask bootstrap pressure")?;
        let evidence = cache::capture(host, bytes, available)?;
        let retained = metadata_bytes(&p.host, &p.boot)
            .checked_add(p.interface_bytes)
            .and_then(|n| n.checked_add(evidence.charged_bytes()))
            .ok_or("Mask cache extent")?;
        if retained > p.baseline_admitted {
            return Err("Mask retained bootstrap pressure".into());
        }
        p.evidence = Some(Rc::new(evidence));
        Ok(())
    })
}
pub(crate) fn freeze() -> Result<(), String> {
    POOL.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.as_mut().ok_or("Mask bootstrap absent")?;
        if p.interface_bytes == 0 || p.evidence.is_none() {
            return Err("Mask bootstrap incomplete".into());
        }
        p.frozen = true;
        Ok(())
    })
}
pub(crate) fn can_reset() -> Result<(), String> {
    POOL.with(|s| {
        if s.borrow().as_ref().is_some_and(|p| {
            p.occupied.iter().any(|v| *v)
                || p.evidence
                    .as_ref()
                    .is_some_and(|e| Rc::strong_count(e) != 1)
        }) {
            return Err("Mask preparation pool has live custody".into());
        }
        Ok(())
    })
}
pub(crate) fn reset() -> Result<(), String> {
    can_reset()?;
    POOL.with(|s| *s.borrow_mut() = None);
    Ok(())
}
pub(crate) fn snapshot(host: &HostId, boot: &BootId) -> Result<Rc<Evidence>, String> {
    POOL.with(|s| {
        let s = s.borrow();
        let p = s.as_ref().ok_or("Mask pool absent")?;
        if !p.frozen || &p.host != host || &p.boot != boot {
            return Err("Mask snapshot incarnation".into());
        }
        p.evidence
            .as_ref()
            .cloned()
            .ok_or_else(|| "Mask snapshot absent".into())
    })
}
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn baseline_admitted() -> usize {
    POOL.with(|s| s.borrow().as_ref().map_or(0, |p| p.baseline_admitted))
}

pub(super) struct Lease {
    slot: usize,
    admitted: Cell<usize>,
}
impl Lease {
    pub(super) const fn metadata_bytes() -> usize {
        std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()
    }
    pub(super) fn acquire_current() -> Result<Rc<Self>, String> {
        POOL.with(|s| {
            let mut s = s.borrow_mut();
            let p = s.as_mut().ok_or("Mask preparation pool absent")?;
            if !p.frozen || p.evidence.is_none() {
                return Err("Mask preparation pool not frozen".into());
            }
            let slot = p
                .occupied
                .iter()
                .position(|v| !*v)
                .ok_or("Mask preparation pool pressure")?;
            p.occupied[slot] = true;
            Ok(Rc::new(Self {
                slot,
                admitted: Cell::new(Self::metadata_bytes()),
            }))
        })
    }
    pub(super) fn matches(&self, host: &HostId, boot: &BootId) -> bool {
        ready(host, boot)
    }
    /// Total client model/workspace charge, including metadata_bytes() once.
    pub(super) fn admit(&self, bytes: usize) -> Result<(), String> {
        if bytes > profile::PREPARATION_BYTES as usize {
            return Err("Mask preparation bytes pressure".into());
        }
        self.admitted.set(self.admitted.get().max(bytes));
        Ok(())
    }
    pub(super) fn admitted(&self) -> usize {
        self.admitted.get()
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        POOL.with(|s| {
            if let Some(p) = s.borrow_mut().as_mut() {
                p.occupied[self.slot] = false;
            }
        });
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "full_profile_pool/tests.rs"]
mod tests;
