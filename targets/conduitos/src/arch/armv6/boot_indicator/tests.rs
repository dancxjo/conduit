use super::*;
use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;
#[derive(Default)]
struct State {
    function: u32,
    writes: Vec<(usize, u32)>,
}
struct Window(Rc<RefCell<State>>);
unsafe impl Registers for Window {
    fn read(&mut self, offset: usize) -> u32 {
        assert_eq!(offset, FUNCTION);
        self.0.borrow().function
    }
    fn write(&mut self, offset: usize, value: u32) {
        let mut state = self.0.borrow_mut();
        state.writes.push((offset, value));
        if offset == FUNCTION {
            state.function = value;
        }
    }
}
fn window(function: u32) -> (Window, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State {
        function,
        ..State::default()
    }));
    (Window(state.clone()), state)
}
#[test]
fn firmware_input_or_output_handoff_latches_low_before_output_without_touching_neighbors() {
    for prior in [0, OUTPUT] {
        let other = 0xa51c9a5b & !FUNCTION_MASK;
        let (window, state) = window(other | prior);
        let mut led = Indicator::admit(window).unwrap();
        assert_eq!(
            state.borrow().writes,
            [(CLEAR, LED), (FUNCTION, other | OUTPUT)]
        );
        led.set(true).unwrap();
        assert_eq!(state.borrow().writes.last(), Some(&(SET, LED)));
        led.revoke();
        assert_eq!(state.borrow().function, other | OUTPUT);
        assert_eq!(state.borrow().writes.last(), Some(&(CLEAR, LED)));
        let writes = state.borrow().writes.len();
        assert_eq!(led.set(true), Err(Refusal::Revoked));
        drop(led);
        assert_eq!(state.borrow().writes.len(), writes);
    }
}
#[test]
fn unknown_alternate_functions_refuse_without_any_gpio_write() {
    for alternate in 2..8 {
        let (window, state) = window(alternate << SHIFT);
        assert!(matches!(
            Indicator::admit(window),
            Err(Refusal::AlternateFunction)
        ));
        assert!(state.borrow().writes.is_empty());
    }
}
#[test]
fn lost_function_prevents_drive_and_drop_preserves_unknown_owner() {
    let (window, state) = window(0);
    let mut led = Indicator::admit(window).unwrap();
    state.borrow_mut().function = 4 << SHIFT;
    let writes = state.borrow().writes.len();
    assert_eq!(led.set(true), Err(Refusal::OwnershipLost));
    drop(led);
    assert_eq!(state.borrow().function, 4 << SHIFT);
    assert_eq!(state.borrow().writes.len(), writes);
}
#[test]
fn drop_drives_only_owned_led_low_and_keeps_safe_output_mode() {
    let (window, state) = window(0);
    let mut led = Indicator::admit(window).unwrap();
    led.set(true).unwrap();
    drop(led);
    assert_eq!(state.borrow().function, OUTPUT);
    assert_eq!(state.borrow().writes.last(), Some(&(CLEAR, LED)));
}
