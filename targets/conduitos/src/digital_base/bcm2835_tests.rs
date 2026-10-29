use super::*;
use std::{cell::RefCell, rc::Rc, vec::Vec};
#[derive(Default)]
struct State {
    words: [u32; 16],
    writes: Vec<(usize, u32)>,
    reads: usize,
}
#[derive(Clone, Default)]
struct Mock(Rc<RefCell<State>>);
// SAFETY: this is an isolated single-owner register model with no hardware.
unsafe impl Registers for Mock {
    fn read(&mut self, offset: usize) -> u32 {
        let mut s = self.0.borrow_mut();
        s.reads += 1;
        s.words[offset / 4]
    }
    fn write(&mut self, offset: usize, value: u32) {
        let mut s = self.0.borrow_mut();
        s.writes.push((offset, value));
        match offset {
            SET => s.words[LEVEL / 4] |= value,
            0x20 => s.words[LEVEL / 4 + 1] |= value,
            CLEAR => s.words[LEVEL / 4] &= !value,
            0x2c => s.words[LEVEL / 4 + 1] &= !value,
            _ => s.words[offset / 4] = value,
        }
    }
}
#[test]
fn invalid_pin_never_reads_or_writes_registers() {
    let mock = Mock::default();
    assert!(matches!(
        Pin::admit(mock.clone(), 54, Direction::Input),
        Err(AdmissionRefusal::InvalidPin)
    ));
    assert_eq!(mock.0.borrow().reads, 0);
    assert!(mock.0.borrow().writes.is_empty());
}
#[test]
fn input_observes_only_the_selected_contact_and_rejects_drive() {
    for index in 0..54_u8 {
        let mock = Mock::default();
        let mut pin = Pin::admit(mock.clone(), index, Direction::Input).unwrap();
        assert_eq!(pin.access(DigitalRequest::Observe), Ok(false));
        mock.0.borrow_mut().words[(LEVEL + usize::from(index / 32) * 4) / 4] = 1 << (index % 32);
        assert_eq!(pin.access(DigitalRequest::Observe), Ok(true));
        assert_eq!(
            pin.access(DigitalRequest::Drive(false)),
            Err(DigitalDisposition::Unsupported)
        );
        drop(pin);
        assert!(mock.0.borrow().writes.is_empty());
    }
}
#[test]
fn output_latch_is_set_before_direction_and_unrelated_functions_survive() {
    let mock = Mock::default();
    // Pin20 is an unrelated alternate function; selected pin22 starts as input.
    mock.0.borrow_mut().words[2] = 4;
    let mut pin = Pin::admit(
        mock.clone(),
        22,
        Direction::Output {
            initial_high: true,
            safe_high: false,
        },
    )
    .unwrap();
    assert_eq!(mock.0.borrow().writes, [(SET, 1 << 22), (8, 4 | (1 << 6))]);
    assert_eq!(pin.access(DigitalRequest::Observe), Ok(true));
    assert_eq!(pin.access(DigitalRequest::Drive(false)), Ok(false));
    assert_eq!(mock.0.borrow().writes.last(), Some(&(CLEAR, 1 << 22)));
    assert_eq!(mock.0.borrow().words[2], 4 | (1 << 6));
}
#[test]
fn second_bank_uses_exact_mask_and_register_offsets() {
    let mock = Mock::default();
    let mut pin = Pin::admit(
        mock.clone(),
        53,
        Direction::Output {
            initial_high: false,
            safe_high: true,
        },
    )
    .unwrap();
    assert_eq!(mock.0.borrow().writes, [(CLEAR + 4, 1 << 21), (20, 1 << 9)]);
    assert_eq!(pin.access(DigitalRequest::Drive(true)), Ok(true));
    assert_eq!(mock.0.borrow().writes.last(), Some(&(SET + 4, 1 << 21)));
}
#[test]
fn existing_output_or_alternate_function_is_never_taken_over() {
    for function in 1..8_u32 {
        let mock = Mock::default();
        mock.0.borrow_mut().words[2] = function << 6;
        assert!(matches!(
            Pin::admit(mock.clone(), 22, Direction::Input),
            Err(AdmissionRefusal::Busy)
        ));
        assert!(mock.0.borrow().writes.is_empty());
    }
}
#[test]
fn revoke_and_drop_hold_the_safe_level_and_are_terminal() {
    for safe_high in [false, true] {
        let mock = Mock::default();
        let mut pin = Pin::admit(
            mock.clone(),
            25,
            Direction::Output {
                initial_high: !safe_high,
                safe_high,
            },
        )
        .unwrap();
        pin.revoke();
        assert_eq!(
            mock.0.borrow().writes.last(),
            Some(&(if safe_high { SET } else { CLEAR }, 1 << 25))
        );
        let writes = mock.0.borrow().writes.len();
        assert_eq!(
            pin.access(DigitalRequest::Drive(!safe_high)),
            Err(DigitalDisposition::ProviderLost)
        );
        drop(pin);
        assert_eq!(mock.0.borrow().writes.len(), writes);
        assert_eq!(mock.0.borrow().words[2] >> (5 * 3) & 7, 1);
    }
}
#[test]
fn unexpected_function_change_is_loss_without_overwriting_another_owner() {
    let mock = Mock::default();
    let mut pin = Pin::admit(
        mock.clone(),
        25,
        Direction::Output {
            initial_high: true,
            safe_high: false,
        },
    )
    .unwrap();
    mock.0.borrow_mut().words[2] = 4 << (5 * 3);
    let writes = mock.0.borrow().writes.len();
    assert_eq!(
        pin.access(DigitalRequest::Drive(false)),
        Err(DigitalDisposition::ProviderLost)
    );
    pin.revoke();
    drop(pin);
    assert_eq!(mock.0.borrow().writes.len(), writes);
}
#[test]
fn dropping_an_active_owner_drives_its_safe_level() {
    let mock = Mock::default();
    let pin = Pin::admit(
        mock.clone(),
        22,
        Direction::Output {
            initial_high: true,
            safe_high: false,
        },
    )
    .unwrap();
    drop(pin);
    assert_eq!(mock.0.borrow().writes.last(), Some(&(CLEAR, 1 << 22)));
}
