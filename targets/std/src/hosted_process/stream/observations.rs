//! Thread-local observations of actual subprocess IO and completed reaping.
use std::cell::RefCell;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Event {
    pub pid: u32,
    pub stdout_bytes: usize,
    pub reaped: Option<bool>,
}
type Observer = Box<dyn FnMut(Event)>;
thread_local! {
    static OBSERVER: RefCell<Option<Observer>> = RefCell::new(None);
}
pub(crate) struct Guard;
pub(crate) fn observe(callback: impl FnMut(Event) + 'static) -> Guard {
    OBSERVER.with(|slot| {
        assert!(slot.borrow().is_none(), "one observer per test thread");
        *slot.borrow_mut() = Some(Box::new(callback));
    });
    Guard
}
pub(super) fn emit(event: Event) {
    OBSERVER.with(|slot| {
        if let Some(callback) = slot.borrow_mut().as_mut() {
            callback(event);
        }
    });
}
impl Drop for Guard {
    fn drop(&mut self) {
        OBSERVER.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}
