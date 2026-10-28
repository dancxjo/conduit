//! Finite BCM2835 hardware RNG access beneath the admitted entropy Base.
//!
//! Register facts reviewed against Linux `drivers/char/hw_random/bcm2835-rng.c`:
//! CTRL 0, STATUS 4, DATA 8, enable bit 0, available words in STATUS[31:24],
//! and a 0x40000 warm-up count before enabling. This is an independent Rust
//! implementation, not a port of the Linux driver. Poll bounds limit register
//! work, not elapsed time or a measured entropy-quality estimate.

use super::{
    CryptographicEntropySource, EntropyProvider, EntropyRefusal, MAXIMUM_SECRET_BYTES, erase,
};

const CTRL: usize = 0;
const STATUS: usize = 4;
const DATA: usize = 8;
const ENABLE: u32 = 1;
const WARMUP: u32 = 0x40000;

/// Exclusive, ordered machine-owned RNG register access.
///
/// # Safety
/// The implementation must own one BCM2835 RNG register window exclusively,
/// including against firmware, interrupt and DMA access, throughout its
/// lifetime. MMIO mapping, device ordering and completion remain machine facts.
pub unsafe trait Registers {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
}

/// Retains the exact source identity and one immutable per-request poll budget.
/// No software randomness, retry admission, allocation or weak fallback is used.
pub struct Generator<R: Registers> {
    registers: R,
    provider: EntropyProvider,
    maximum_polls: u32,
    owned: bool,
    original_control: u32,
}

impl<R: Registers> Generator<R> {
    pub fn admit(
        mut registers: R,
        provider: EntropyProvider,
        maximum_polls: u32,
    ) -> Result<Self, EntropyRefusal> {
        if !provider.is_valid() || maximum_polls == 0 {
            return Err(EntropyRefusal::InvalidProvider);
        }
        let control = registers.read(CTRL);
        if control & ENABLE != 0 {
            // Admission must not stop or reset a preexisting owner.
            return Err(EntropyRefusal::Unavailable);
        }
        registers.write(STATUS, WARMUP);
        registers.write(CTRL, control | ENABLE);
        Ok(Self {
            registers,
            provider,
            maximum_polls,
            owned: true,
            original_control: control,
        })
    }

    pub fn revoke(&mut self) {
        if self.owned {
            // If control changed, possession is no longer established. Leave
            // the unknown state untouched rather than clobbering a new owner.
            if self.registers.read(CTRL) == self.original_control | ENABLE {
                self.registers.write(CTRL, self.original_control);
            }
            self.owned = false;
        }
    }

    fn fill_words(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        if !self.owned {
            return Err(EntropyRefusal::Unavailable);
        }
        if output.is_empty() {
            return Err(EntropyRefusal::EmptyBuffer);
        }
        if output.len() > MAXIMUM_SECRET_BYTES {
            return Err(EntropyRefusal::BufferTooLarge);
        }
        let mut polls = self.maximum_polls;
        for chunk in output.chunks_mut(4) {
            loop {
                if polls == 0 {
                    return Err(EntropyRefusal::SourceFailure);
                }
                polls -= 1;
                if self.registers.read(CTRL) != (self.original_control | ENABLE) {
                    return Err(EntropyRefusal::SourceFailure);
                }
                if self.registers.read(STATUS) >> 24 != 0 {
                    let word = self.registers.read(DATA).to_le_bytes();
                    chunk.copy_from_slice(&word[..chunk.len()]);
                    break;
                }
                core::hint::spin_loop();
            }
        }
        Ok(())
    }
}

impl<R: Registers> CryptographicEntropySource for Generator<R> {
    fn provider(&self) -> EntropyProvider {
        self.provider
    }

    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        if let Err(error) = self.fill_words(output) {
            erase(output);
            self.revoke();
            return Err(error);
        }
        Ok(())
    }
}

impl<R: Registers> Drop for Generator<R> {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cryptographic_entropy::CryptographicEntropyBase;
    use alloc::{rc::Rc, vec::Vec};
    use core::cell::RefCell;

    #[derive(Default)]
    struct State {
        control: u32,
        words: Vec<u32>,
        next: usize,
        statuses: u32,
        writes: Vec<(usize, u32)>,
    }
    struct Fixture(Rc<RefCell<State>>);
    unsafe impl Registers for Fixture {
        fn read(&mut self, offset: usize) -> u32 {
            let mut state = self.0.borrow_mut();
            match offset {
                CTRL => state.control,
                STATUS => {
                    state.statuses += 1;
                    u32::from(state.next < state.words.len()) << 24
                }
                DATA => {
                    let word = state.words[state.next];
                    state.next += 1;
                    word
                }
                _ => panic!("unexpected register"),
            }
        }
        fn write(&mut self, offset: usize, value: u32) {
            let mut state = self.0.borrow_mut();
            state.writes.push((offset, value));
            if offset == CTRL {
                state.control = value;
            }
        }
    }
    const PROVIDER: EntropyProvider = EntropyProvider {
        base_id: "test/bcm2835/rng",
        provider_instance_id: "test/rng/0",
        provider_generation: 1,
    };
    fn fixture(words: &[u32]) -> (Fixture, Rc<RefCell<State>>) {
        let state = Rc::new(RefCell::new(State {
            words: words.to_vec(),
            ..State::default()
        }));
        (Fixture(state.clone()), state)
    }

    #[test]
    fn warmup_precedes_enable_and_exact_words_include_partial_tail() {
        let (registers, state) = fixture(&[0x04030201, 0x08070605]);
        let mut source = Generator::admit(registers, PROVIDER, 2).unwrap();
        assert_eq!(source.provider(), PROVIDER);
        assert_eq!(state.borrow().writes, [(STATUS, WARMUP), (CTRL, ENABLE)]);
        let mut output = [0; 7];
        source.fill_exact(&mut output).unwrap();
        assert_eq!(output, [1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(state.borrow().statuses, 2);
        drop(source);
        assert_eq!(state.borrow().control, 0);
    }

    #[test]
    fn timeout_is_one_request_budget_and_revokes_without_fallback() {
        let (registers, state) = fixture(&[]);
        let mut source = Generator::admit(registers, PROVIDER, 3).unwrap();
        let mut output = [9; 4];
        assert_eq!(
            source.fill_exact(&mut output),
            Err(EntropyRefusal::SourceFailure)
        );
        assert_eq!(output, [0; 4]);
        assert_eq!(state.borrow().statuses, 3);
        assert_eq!(state.borrow().control, 0);
        let writes = state.borrow().writes.len();
        assert_eq!(
            source.fill_exact(&mut output),
            Err(EntropyRefusal::Unavailable)
        );
        source.revoke();
        drop(source);
        assert_eq!(state.borrow().writes.len(), writes);
    }

    #[test]
    fn preexisting_owner_is_never_changed() {
        let (registers, state) = fixture(&[]);
        state.borrow_mut().control = ENABLE | 0x80;
        assert!(matches!(
            Generator::admit(registers, PROVIDER, 1),
            Err(EntropyRefusal::Unavailable)
        ));
        assert!(state.borrow().writes.is_empty());
        assert_eq!(state.borrow().control, ENABLE | 0x80);
    }

    #[test]
    fn partial_timeout_erases_through_the_existing_base_and_consumes_capacity() {
        let (registers, state) = fixture(&[0x04030201]);
        let source = Generator::admit(registers, PROVIDER, 2).unwrap();
        let mut base = CryptographicEntropyBase::<_, 1>::admit(source).unwrap();
        let mut output = [9; 8];
        assert_eq!(base.fill(&mut output), Err(EntropyRefusal::SourceFailure));
        assert_eq!(output, [0; 8]);
        assert_eq!(base.requests_used(), 1);
        assert_eq!(state.borrow().next, 1);
        assert_eq!(state.borrow().statuses, 2);
        assert_eq!(state.borrow().control, 0);
        assert_eq!(base.fill(&mut output), Err(EntropyRefusal::RequestCapacity));
    }

    #[test]
    fn explicit_revoke_is_idempotent_and_preserves_other_control_bits() {
        let (registers, state) = fixture(&[]);
        state.borrow_mut().control = 0x80;
        let mut source = Generator::admit(registers, PROVIDER, 1).unwrap();
        source.revoke();
        assert_eq!(state.borrow().control, 0x80);
        let writes = state.borrow().writes.len();
        source.revoke();
        drop(source);
        assert_eq!(state.borrow().writes.len(), writes);
    }
    #[test]
    fn changed_control_is_preserved_when_the_original_owner_retires() {
        let (registers, state) = fixture(&[0x04030201]);
        let mut source = Generator::admit(registers, PROVIDER, 1).unwrap();
        state.borrow_mut().control = ENABLE | 0x80;
        let writes = state.borrow().writes.len();
        let mut output = [9; 4];
        assert_eq!(
            source.fill_exact(&mut output),
            Err(EntropyRefusal::SourceFailure)
        );
        assert_eq!(output, [0; 4]);
        assert_eq!(state.borrow().next, 0);
        drop(source);
        assert_eq!(state.borrow().control, ENABLE | 0x80);
        assert_eq!(state.borrow().writes.len(), writes);
    }
}
