//! Exact electrical display custody; possession alone grants no Play authority.
use core::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};
use critical_section::Mutex;
use static_cell::StaticCell;
pub type Display = crate::tm1637_pins::Pins;
static OWNER: StaticCell<Display> = StaticCell::new();
static SLOT: Mutex<RefCell<Option<&'static mut Display>>> = Mutex::new(RefCell::new(None));
pub fn initialize(display: Display) {
    let display = OWNER.init(display);
    critical_section::with(|cs| *SLOT.borrow(cs).borrow_mut() = Some(display));
}
pub struct DisplayLease(Option<&'static mut Display>);
impl Deref for DisplayLease {
    type Target = Display;
    fn deref(&self) -> &Display {
        self.0.as_deref().expect("live display lease")
    }
}
impl DerefMut for DisplayLease {
    fn deref_mut(&mut self) -> &mut Display {
        self.0.as_deref_mut().expect("live display lease")
    }
}
impl Drop for DisplayLease {
    fn drop(&mut self) {
        if let Some(display) = self.0.take() {
            critical_section::with(|cs| {
                let mut slot = SLOT.borrow(cs).borrow_mut();
                debug_assert!(slot.is_none());
                *slot = Some(display);
            });
        }
    }
}
pub fn try_lease() -> Option<DisplayLease> {
    critical_section::with(|cs| {
        SLOT.borrow(cs)
            .borrow_mut()
            .take()
            .map(|display| DisplayLease(Some(display)))
    })
}
