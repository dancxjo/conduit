//! Initialized computation inventory; neither a work pool nor an active Plot.
use core::cell::Cell;
use critical_section::Mutex;
use static_cell::StaticCell;
#[allow(dead_code)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/resident.rs"));
}
static OWNER: StaticCell<generated::Initialized> = StaticCell::new();
static INVENTORY: Mutex<Cell<Option<&'static generated::Initialized>>> =
    Mutex::new(Cell::new(None));
static REFUSAL: Mutex<Cell<Option<usize>>> = Mutex::new(Cell::new(None));

/// Called once at boot; a failed member keeps the whole inventory unpublished.
pub fn initialize() {
    match generated::Initialized::prepare() {
        Ok(owner) => {
            let owner = OWNER.init(owner);
            critical_section::with(|cs| INVENTORY.borrow(cs).set(Some(owner)));
        }
        Err(refusal) => {
            critical_section::with(|cs| REFUSAL.borrow(cs).set(Some(refusal.entry())));
        }
    }
}
pub fn summary() -> (usize, usize, Option<usize>) {
    critical_section::with(|cs| {
        let count = INVENTORY
            .borrow(cs)
            .get()
            .map_or(0, |owner| owner.offers().len());
        (
            count,
            core::mem::size_of::<generated::Initialized>(),
            REFUSAL.borrow(cs).get(),
        )
    })
}
