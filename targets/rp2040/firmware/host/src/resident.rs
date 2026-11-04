//! Initialized computation custody; neither a work pool nor an active Plot.
use core::{
    cell::{Cell, RefCell},
    ops::{Deref, DerefMut},
};
use critical_section::Mutex;
use static_cell::StaticCell;
#[allow(dead_code)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/resident.rs"));
}
static OWNER: StaticCell<generated::Initialized> = StaticCell::new();
static INVENTORY: Mutex<RefCell<Option<&'static mut generated::Initialized>>> =
    Mutex::new(RefCell::new(None));
static INITIALIZED: Mutex<Cell<usize>> = Mutex::new(Cell::new(0));
static REFUSAL: Mutex<Cell<Option<usize>>> = Mutex::new(Cell::new(None));

/// Called once at boot; a failed member keeps the whole inventory unpublished.
pub fn initialize() {
    match generated::Initialized::prepare() {
        Ok(owner) => {
            let owner = OWNER.init(owner);
            let count = owner.offers().into_iter().flatten().count();
            critical_section::with(|cs| {
                INITIALIZED.borrow(cs).set(count);
                *INVENTORY.borrow(cs).borrow_mut() = Some(owner);
            });
        }
        Err(refusal) => {
            critical_section::with(|cs| REFUSAL.borrow(cs).set(Some(refusal.entry())));
        }
    }
}
/// Completed initialization is observable even while its owner is exclusively
/// borrowed. This summary never asserts availability, a reservation or a Play.
pub fn summary() -> (usize, usize, Option<usize>) {
    critical_section::with(|cs| {
        (
            INITIALIZED.borrow(cs).get(),
            core::mem::size_of::<generated::Initialized>(),
            REFUSAL.borrow(cs).get(),
        )
    })
}

/// Distinct local acquisition failures; none grants execution authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryRefusal {
    NotInitialized,
    InitializationFailed(usize),
    Busy,
}

/// Exclusive Rust custody only. Root must still check assignment authority,
/// reserve actual resources and quiesce prior execution before binding a Back.
/// The inventory stays in its single static allocation; async frames retain
/// only this pointer. References borrowed through the lease cannot outlive it.
pub struct InventoryLease {
    owner: Option<&'static mut generated::Initialized>,
}
impl Deref for InventoryLease {
    type Target = generated::Initialized;
    fn deref(&self) -> &Self::Target {
        self.owner.as_deref().expect("live inventory lease")
    }
}
impl DerefMut for InventoryLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.owner.as_deref_mut().expect("live inventory lease")
    }
}
impl Drop for InventoryLease {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            critical_section::with(|cs| {
                let mut slot = INVENTORY.borrow(cs).borrow_mut();
                // Only initialize-once and this non-clonable lease can fill it.
                debug_assert!(slot.is_none());
                *slot = Some(owner);
            });
        }
    }
}
/// Busy or failed initialization refuses without moving the retained Backs.
/// Cancellation, refusal and ordinary completion all return the exclusive loan.
pub fn inventory() -> Result<InventoryLease, InventoryRefusal> {
    critical_section::with(|cs| {
        if let Some(owner) = INVENTORY.borrow(cs).borrow_mut().take() {
            return Ok(InventoryLease { owner: Some(owner) });
        }
        Err(match REFUSAL.borrow(cs).get() {
            Some(entry) => InventoryRefusal::InitializationFailed(entry),
            None if INITIALIZED.borrow(cs).get() == 0 => InventoryRefusal::NotInitialized,
            None => InventoryRefusal::Busy,
        })
    })
}
