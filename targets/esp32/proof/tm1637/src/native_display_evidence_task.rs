//! Single-executor page custody. Root inspection is read-only; only an exact
//! authenticated owner acceptance frees the page and wakes the display worker.
use crate::{
    native_display_evidence::{Custody, Log},
    native_owner_control::EvidenceReference,
};
use core::cell::RefCell;
use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    signal::Signal,
};
static PAGE: Mutex<CriticalSectionRawMutex, RefCell<Option<Custody>>> =
    Mutex::new(RefCell::new(None));
static ACCEPTED: Signal<CriticalSectionRawMutex, EvidenceReference> = Signal::new();
pub fn prepare() -> Result<(), &'static str> {
    PAGE.lock(|cell| {
        let mut page = cell.borrow_mut();
        if page.is_some() {
            return Err("display-evidence-already-prepared");
        }
        *page = Some(Custody::prepare());
        Ok(())
    })
}
pub fn publish(invocation: u64, log: &Log) -> Result<Option<EvidenceReference>, &'static str> {
    PAGE.lock(|cell| {
        let mut page = cell.borrow_mut();
        let page = page.as_mut().ok_or("display-evidence-unprepared")?;
        if page.pending().is_none() {
            page.publish(invocation, log)?;
        }
        let pending = page.pending();
        #[cfg(feature = "local-evidence-store")]
        if let Some(reference) = pending {
            crate::native_evidence_archive::append(
                page.retained_bytes().ok_or("evidence-local-bytes")?,
            )?;
            page.accept(reference)?;
            let accepted = page.release_accepted()?;
            ACCEPTED.signal(accepted);
        }
        Ok(pending)
    })
}
pub fn copy(output: &mut [u8]) -> Result<Option<(EvidenceReference, usize)>, &'static str> {
    #[cfg(feature = "local-evidence-store")]
    if let Some(page) = crate::native_evidence_archive::copy(output)? {
        return Ok(Some(page));
    }
    PAGE.lock(|cell| {
        cell.borrow()
            .as_ref()
            .ok_or("display-evidence-unprepared")?
            .copy(output)
    })
}
pub fn pending() -> bool {
    #[cfg(feature = "local-evidence-store")]
    if crate::native_evidence_archive::pending() {
        return true;
    }
    PAGE.lock(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|p| p.pending().is_some())
    })
}
pub fn accept(reference: EvidenceReference) -> Result<(), &'static str> {
    #[cfg(feature = "local-evidence-store")]
    if crate::native_evidence_archive::accept(reference).is_ok() {
        return Ok(());
    }
    PAGE.lock(|cell| {
        let mut page = cell.borrow_mut();
        let page = page.as_mut().ok_or("display-evidence-unprepared")?;
        page.accept(reference)?;
        let accepted = page.release_accepted()?;
        ACCEPTED.signal(accepted);
        Ok(())
    })
}
pub async fn wait(reference: EvidenceReference) -> Result<(), &'static str> {
    if ACCEPTED.wait().await != reference {
        return Err("display-evidence-wake-scope");
    }
    Ok(())
}
