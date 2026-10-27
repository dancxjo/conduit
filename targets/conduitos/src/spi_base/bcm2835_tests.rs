use super::*;
use std::{cell::RefCell, collections::VecDeque, rc::Rc, vec::Vec};

#[derive(Default)]
struct State {
    control: u32,
    writes: Vec<(usize, u32)>,
    receive: VecDeque<u8>,
    sent: Vec<u8>,
    no_tx: bool,
    no_rx: bool,
    no_done: bool,
}
#[derive(Clone, Default)]
struct Mock(Rc<RefCell<State>>);
// SAFETY: This isolated model has one controller owner and no hardware effects.
unsafe impl Registers for Mock {
    fn read(&mut self, offset: usize) -> u32 {
        let mut s = self.0.borrow_mut();
        match offset {
            CS => {
                s.control
                    | if s.no_tx { 0 } else { TXD }
                    | if s.receive.is_empty() { 0 } else { RXD }
                    | if s.no_done || !s.receive.is_empty() {
                        0
                    } else {
                        DONE
                    }
            }
            FIFO => u32::from(s.receive.pop_front().expect("RXD required")),
            _ => panic!("unexpected register"),
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        let mut s = self.0.borrow_mut();
        s.writes.push((offset, value));
        match offset {
            CS => {
                s.control = value & !CLEAR;
                if value & CLEAR != 0 {
                    s.receive.clear();
                }
            }
            CLK => (),
            FIFO => {
                s.sent.push(value as u8);
                if !s.no_rx {
                    s.receive.push_back(value as u8 ^ 0xff);
                }
            }
            _ => panic!("unexpected register"),
        }
    }
}
fn configuration() -> Configuration {
    Configuration {
        chip_select: 0,
        clock_polarity: false,
        clock_phase: false,
        clock_divider: 64,
        maximum_bytes: 16,
        maximum_polls: 64,
    }
}
#[test]
fn exchange_drains_rx_and_waits_for_shift_completion() {
    let mock = Mock::default();
    let mut controller = Controller::admit(mock.clone(), configuration()).unwrap();
    let mut received = [0; 4];
    controller
        .exchange(&[0, 0x12, 0x34, 0xff], &mut received)
        .unwrap();
    assert_eq!(received, [0xff, 0xed, 0xcb, 0]);
    assert_eq!(mock.0.borrow().sent, [0, 0x12, 0x34, 0xff]);
    assert_eq!(mock.0.borrow().control & TA, 0);
    assert_eq!(mock.0.borrow().writes.last(), Some(&(CS, CLEAR)));
    // Successful completion does not revoke the admitted controller.
    controller.exchange(&[0x55], &mut [0]).unwrap();
}
#[test]
fn all_three_stalls_stop_and_permanently_revoke() {
    for stalled in 0..3 {
        let mock = Mock::default();
        {
            let mut state = mock.0.borrow_mut();
            state.no_tx = stalled == 0;
            state.no_rx = stalled == 1;
            state.no_done = stalled == 2;
        }
        let mut controller = Controller::admit(mock.clone(), configuration()).unwrap();
        assert_eq!(
            controller.exchange(&[0x11, 0x22], &mut [0; 2]),
            Err(Refusal::TimedOut)
        );
        assert_eq!(mock.0.borrow().control & TA, 0);
        assert_eq!(mock.0.borrow().writes.last(), Some(&(CS, CLEAR)));
        let writes = mock.0.borrow().writes.len();
        assert_eq!(
            controller.exchange(&[0], &mut [0]),
            Err(Refusal::ProviderLost)
        );
        assert_eq!(mock.0.borrow().writes.len(), writes);
        assert_eq!(mock.0.borrow().sent.len(), if stalled == 0 { 0 } else { 2 });
    }
}
#[test]
fn busy_admission_never_touches_registers() {
    let mock = Mock::default();
    mock.0.borrow_mut().control = TA;
    assert!(matches!(
        Controller::admit(mock.clone(), configuration()),
        Err(Refusal::Busy)
    ));
    assert!(mock.0.borrow().writes.is_empty());
}
#[test]
fn invalid_configuration_has_no_effects() {
    for field in 0..5 {
        let mock = Mock::default();
        let mut config = configuration();
        match field {
            0 => config.maximum_bytes = 0,
            1 => config.maximum_bytes = 4097,
            2 => config.maximum_polls = 0,
            3 => config.clock_divider = 3,
            _ => config.chip_select = 2,
        }
        assert!(Controller::admit(mock.clone(), config).is_err());
        assert!(mock.0.borrow().writes.is_empty());
    }
}
#[test]
fn bad_request_never_starts_and_does_not_revoke() {
    let mock = Mock::default();
    let mut controller = Controller::admit(mock.clone(), configuration()).unwrap();
    let writes = mock.0.borrow().writes.len();
    for (output, input) in [
        (&[][..], &mut [][..]),
        (&[0][..], &mut [0; 2][..]),
        (&[0; 17][..], &mut [0; 17][..]),
    ] {
        assert_eq!(
            controller.exchange(output, input),
            Err(Refusal::InvalidGeometry)
        );
    }
    assert_eq!(mock.0.borrow().writes.len(), writes);
    controller.exchange(&[1], &mut [0]).unwrap();
}
#[test]
fn select_mode_and_slowest_divider_are_exact() {
    let mock = Mock::default();
    let mut config = configuration();
    config.chip_select = 1;
    config.clock_phase = true;
    config.clock_polarity = true;
    config.clock_divider = 0;
    let mut controller = Controller::admit(mock.clone(), config).unwrap();
    assert_eq!(mock.0.borrow().writes, [(CS, 13 | CLEAR), (CLK, 0)]);
    controller.exchange(&[1], &mut [0]).unwrap();
    assert_eq!(mock.0.borrow().control, 13);
    drop(controller);
    assert_eq!(mock.0.borrow().writes.last(), Some(&(CS, 13 | CLEAR)));
}
#[test]
fn explicit_revoke_is_terminal() {
    let mock = Mock::default();
    let mut controller = Controller::admit(mock.clone(), configuration()).unwrap();
    controller.revoke();
    assert_eq!(
        controller.exchange(&[1], &mut [0]),
        Err(Refusal::ProviderLost)
    );
    assert_eq!(mock.0.borrow().control & TA, 0);
}
#[test]
fn changed_busy_state_refuses_without_overwriting_unknown_owner() {
    let mock = Mock::default();
    let mut controller = Controller::admit(mock.clone(), configuration()).unwrap();
    mock.0.borrow_mut().control |= TA;
    let writes = mock.0.borrow().writes.len();
    assert_eq!(
        controller.exchange(&[1], &mut [0]),
        Err(Refusal::ProviderLost)
    );
    assert_eq!(mock.0.borrow().writes.len(), writes);
    controller.revoke();
    drop(controller);
    assert_eq!(mock.0.borrow().writes.len(), writes);
}

#[test]
fn exhausted_work_budget_never_reports_a_partial_exchange_as_success() {
    let mock = Mock::default();
    let mut config = configuration();
    config.maximum_polls = 2;
    let mut controller = Controller::admit(mock.clone(), config).unwrap();
    let mut input = [0x55; 4];
    assert_eq!(
        controller.exchange(&[0, 1, 2, 3], &mut input),
        Err(Refusal::TimedOut)
    );
    assert_eq!(mock.0.borrow().sent, [0, 1]);
    assert_eq!(input, [0xff, 0x55, 0x55, 0x55]);
    assert_eq!(mock.0.borrow().control & TA, 0);
    assert!(mock.0.borrow().receive.is_empty());
}
