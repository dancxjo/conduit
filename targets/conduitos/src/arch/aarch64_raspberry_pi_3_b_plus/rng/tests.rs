use super::*;
use std::collections::VecDeque;

struct Mock {
    control: u32,
    status: VecDeque<u32>,
    words: VecDeque<u32>,
    writes: Vec<(usize, u32)>,
    status_reads: u32,
    data_reads: u32,
    control_reads: u32,
    control_failure: Option<u32>,
}
impl Mock {
    fn ready() -> Self {
        Self {
            control: 0,
            status: [1 << 24; SAMPLE_WORDS].into(),
            words: (1..=SAMPLE_WORDS as u32).collect(),
            writes: Vec::new(),
            status_reads: 0,
            data_reads: 0,
            control_reads: 0,
            control_failure: None,
        }
    }
    fn stopped(&self) {
        assert_eq!(self.control, 0);
        assert_eq!(self.writes.last(), Some(&(CTRL, 0)));
        assert!(self
            .writes
            .iter()
            .all(|(offset, _)| *offset < REGISTER_BYTES));
    }
}
impl Registers for Mock {
    fn read(&mut self, offset: usize) -> u32 {
        match offset {
            CTRL => {
                self.control_reads += 1;
                if self.control_failure == Some(self.control_reads) {
                    0
                } else {
                    self.control
                }
            }
            STATUS => {
                self.status_reads += 1;
                self.status.pop_front().unwrap_or(0)
            }
            DATA => {
                self.data_reads += 1;
                self.words.pop_front().expect("unannounced DATA read")
            }
            _ => panic!("unsupported register"),
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        self.writes.push((offset, value));
        if offset == CTRL {
            self.control = value;
        }
    }
}

#[test]
fn initializes_with_hardware_warmup_and_withholds_first_word() {
    let mut registers = Mock::ready();
    let seed = collect(&mut registers, SAMPLE_WORDS as u32).unwrap();
    let mut expected = [0; SEED_BYTES];
    for (bytes, word) in expected.chunks_exact_mut(4).zip(2_u32..=9) {
        bytes.copy_from_slice(&word.to_le_bytes());
    }
    assert_eq!(seed, expected);
    assert_eq!(
        registers.writes,
        [(CTRL, 0), (STATUS, WARMUP), (CTRL, 1), (CTRL, 0)]
    );
    assert_eq!(registers.status_reads, 9);
    assert_eq!(registers.data_reads, 9);
    registers.stopped();
}

#[test]
fn budget_is_shared_across_warmup_and_all_words() {
    let mut registers = Mock::ready();
    registers.status.push_front(0);
    registers.status.push_front(0);
    assert_eq!(
        collect(&mut registers, 10),
        Err(RngRefusal::PollBudgetExhausted)
    );
    assert_eq!(registers.status_reads, 10);
    assert_eq!(registers.data_reads, 8);
    registers.stopped();
}

#[test]
fn unavailable_fifo_never_reads_data_or_exceeds_budget() {
    let mut registers = Mock::ready();
    registers.status.clear();
    assert_eq!(
        collect(&mut registers, 23),
        Err(RngRefusal::PollBudgetExhausted)
    );
    assert_eq!(registers.status_reads, 23);
    assert_eq!(registers.data_reads, 0);
    registers.stopped();
}

#[test]
fn invalid_budget_touches_no_registers() {
    for budget in [0, MAXIMUM_STATUS_POLLS + 1, u32::MAX] {
        let mut registers = Mock::ready();
        assert_eq!(
            collect(&mut registers, budget),
            Err(RngRefusal::InvalidBudget)
        );
        assert!(registers.writes.is_empty());
        assert_eq!(
            registers.control_reads + registers.status_reads + registers.data_reads,
            0
        );
    }
}

#[test]
fn rejects_disabled_generator_both_before_and_after_collection() {
    for read in [1, 2] {
        let mut registers = Mock::ready();
        registers.control_failure = Some(read);
        assert_eq!(collect(&mut registers, 9), Err(RngRefusal::Unavailable));
        assert_eq!(registers.data_reads, if read == 1 { 0 } else { 9 });
        registers.stopped();
    }
}

#[test]
fn refuses_bus_all_ones_status_without_reading_data() {
    let mut registers = Mock::ready();
    registers.status[0] = u32::MAX;
    assert_eq!(collect(&mut registers, 9), Err(RngRefusal::Unavailable));
    assert_eq!(registers.data_reads, 0);
    registers.stopped();
}

#[test]
fn refuses_zero_all_ones_and_nonadjacent_repetition_without_partial_seed() {
    for (position, value) in [(0, 0), (8, 0), (4, u32::MAX), (1, 1), (8, 2)] {
        let mut registers = Mock::ready();
        registers.words[position] = value;
        assert_eq!(collect(&mut registers, 9), Err(RngRefusal::Unhealthy));
        assert_eq!(registers.data_reads, position as u32 + 1);
        registers.stopped();
    }
}

#[test]
fn delayed_word_is_read_only_after_fifo_availability() {
    let mut registers = Mock::ready();
    registers.status.insert(4, WARMUP - 1);
    assert!(collect(&mut registers, 10).is_ok());
    assert_eq!(registers.status_reads, 10);
    assert_eq!(registers.data_reads, 9);
    registers.stopped();
}

#[test]
fn pointer_checks_are_not_device_or_mapping_attestation() {
    for base in [0, 1, PHYSICAL_BASE + 1, usize::MAX - 3] {
        assert_eq!(validate_mapping(base), Err(RngRefusal::InvalidMapping));
    }
    assert_eq!(validate_mapping(PHYSICAL_BASE), Ok(()));
    // The actual MMU/ownership safety contract remains with the board caller.
    assert_eq!(validate_mapping(0x1000), Ok(()));
}
