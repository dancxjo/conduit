use super::*;

struct Mock {
    words: [u32; 40],
    writes: Vec<(usize, u32)>,
    time: u64,
    step: u64,
    reads: usize,
    counters: usize,
    reject_fsel: bool,
}
impl Default for Mock {
    fn default() -> Self {
        Self {
            words: [0; 40],
            writes: Vec::new(),
            time: 0,
            step: 0,
            reads: 0,
            counters: 0,
            reject_fsel: false,
        }
    }
}
impl Registers for Mock {
    fn read(&mut self, offset: usize) -> u32 {
        self.reads += 1;
        self.words[offset / 4]
    }
    fn write(&mut self, offset: usize, value: u32) {
        self.writes.push((offset, value));
        if offset != FSEL1 || !self.reject_fsel {
            self.words[offset / 4] = value;
        }
    }
    fn counter(&mut self) -> u64 {
        self.counters += 1;
        let now = self.time;
        self.time = self.time.wrapping_add(self.step);
        now
    }
}

#[test]
fn mapping_rejects_null_misalignment_and_overflow() {
    for base in [0, 1, 3, usize::MAX & !3] {
        assert_eq!(validate_mapping(base), Err(PinRefusal::InvalidMapping));
    }
    assert_eq!(validate_mapping(PHYSICAL_BASE), Ok(()));
}

#[test]
fn invalid_bounds_perform_no_accesses() {
    for (ticks, polls) in [(0, 1), (1, 0), (1, MAXIMUM_DELAY_POLLS + 1)] {
        let mut m = Mock::default();
        assert_eq!(route(&mut m, ticks, polls), Err(PinRefusal::InvalidTiming));
        assert_eq!((m.reads, m.counters, m.writes.len()), (0, 0, 0));
    }
}

#[test]
fn preserves_other_functions_and_only_clocks_selected_pads() {
    let mut m = Mock {
        step: 1,
        ..Mock::default()
    };
    let original = 0x1234_5678;
    m.words[FSEL1 / 4] = original;
    assert_eq!(route(&mut m, 3, 3), Ok(()));
    assert_eq!(
        m.writes,
        vec![
            (PUD, 0),
            (CLK0, PINS),
            (PUD, 0),
            (CLK0, 0),
            (FSEL1, (original & !FIELDS) | ALT0),
        ]
    );
    assert_eq!(m.words[FSEL1 / 4] & !FIELDS, original & !FIELDS);
    assert_eq!(m.counters, 8); // two start reads plus six bounded polls
}

#[test]
fn busy_shared_controller_refuses_without_writing() {
    for (offset, value) in [(PUD, 1), (CLK0, 1), (CLK1, 1)] {
        let mut m = Mock::default();
        m.words[offset / 4] = value;
        assert_eq!(route(&mut m, 1, 1), Err(PinRefusal::PullControllerBusy));
        assert!(m.writes.is_empty());
    }
}

#[test]
fn unavailable_registers_refuse_without_writing() {
    for offset in [PUD, CLK0, CLK1, FSEL1] {
        let mut m = Mock::default();
        m.words[offset / 4] = u32::MAX;
        assert_eq!(route(&mut m, 1, 1), Err(PinRefusal::Unavailable));
        assert!(m.writes.is_empty());
    }
}

#[test]
fn stalled_counter_cleans_controller_without_selecting_uart() {
    let mut m = Mock::default();
    assert_eq!(route(&mut m, 1, 4), Err(PinRefusal::DelayBudgetExhausted));
    assert_eq!(m.counters, 5);
    assert_eq!(m.writes, vec![(PUD, 0), (PUD, 0), (CLK0, 0)]);
}

#[test]
fn insufficient_hold_budget_cleans_clock_without_selecting_uart() {
    struct HoldStall(Mock);
    impl Registers for HoldStall {
        fn read(&mut self, o: usize) -> u32 {
            self.0.read(o)
        }
        fn write(&mut self, o: usize, v: u32) {
            self.0.write(o, v);
            if o == CLK0 && v == PINS {
                self.0.step = 0;
            }
        }
        fn counter(&mut self) -> u64 {
            self.0.counter()
        }
    }
    let mut m = HoldStall(Mock {
        step: 1,
        ..Mock::default()
    });
    assert_eq!(route(&mut m, 1, 2), Err(PinRefusal::DelayBudgetExhausted));
    assert_eq!(
        m.0.writes,
        vec![(PUD, 0), (CLK0, PINS), (PUD, 0), (CLK0, 0)]
    );
}

#[test]
fn readback_mismatch_does_not_report_success() {
    let mut m = Mock {
        step: 1,
        reject_fsel: true,
        ..Mock::default()
    };
    assert_eq!(route(&mut m, 1, 1), Err(PinRefusal::ReadbackMismatch));
}

#[test]
fn counter_wrap_is_supported() {
    let mut m = Mock {
        step: 1,
        time: u64::MAX - 1,
        ..Mock::default()
    };
    assert_eq!(route(&mut m, 3, 3), Ok(()));
}
