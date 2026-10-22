#![no_std]
//! A prepared semantic Face and its bounded HTTP Mask; no second sampler.
extern crate alloc;
use alloc::boxed::Box;
use conduit_environment_face::PreparedObservationFace;
use conduit_face::FaceBasis;
use core::cell::RefCell;
use critical_section::Mutex;
mod mask;
struct Publication {
    face: PreparedObservationFace,
    ap_ready: bool,
}
static PUBLICATION: Mutex<RefCell<Option<Box<Publication>>>> = Mutex::new(RefCell::new(None));
/// Admit the publication grammar and all recurring storage before Play seals.
#[inline(never)]
pub fn initialize(basis: FaceBasis) {
    let face = PreparedObservationFace::prepare(basis, 4096, 30000).unwrap();
    critical_section::with(|cs| {
        let mut slot = PUBLICATION.borrow(cs).borrow_mut();
        assert!(slot.is_none());
        *slot = Some(Box::new(Publication {
            face,
            ap_ready: false,
        }));
    });
}
pub fn set_ap_ready() {
    critical_section::with(|cs| {
        PUBLICATION
            .borrow(cs)
            .borrow_mut()
            .as_deref_mut()
            .unwrap()
            .ap_ready = true
    });
}
pub fn ap_ready() -> bool {
    critical_section::with(|cs| {
        PUBLICATION
            .borrow(cs)
            .borrow()
            .as_deref()
            .is_some_and(|p| p.ap_ready)
    })
}
pub fn response(now: u64, json: bool) -> heapless::String<2048> {
    critical_section::with(|cs| {
        let mut slot = PUBLICATION.borrow(cs).borrow_mut();
        let publication = slot.as_deref_mut().unwrap();
        let ready = publication.ap_ready;
        let face = publication
            .face
            .at(now)
            .expect("monotonic publication clock");
        mask::response(face, ready, now, json)
    })
}

pub fn publish(index: usize, bytes: &[u8], now: u64) {
    critical_section::with(|cs| {
        PUBLICATION
            .borrow(cs)
            .borrow_mut()
            .as_deref_mut()
            .unwrap()
            .face
            .publish_result(index, bytes, now)
            .expect("admitted observation publication");
    });
}

#[cfg(test)]
#[macro_use]
extern crate std;
#[cfg(test)]
mod tests;
