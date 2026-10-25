//! Admission policy for the register snapshot captured by a future Nano entry
//! stub. This does not perform an EL transition or initialize any hardware.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub current_el: u64,
    pub sctlr: u64,
    pub daif: u64,
    pub dtb_address: u64,
    pub reserved_arguments: [u64; 3],
    pub mpidr: u64,
    pub boot_mpidr: u64,
    pub counter_frequency: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    ExceptionLevel,
    TranslationEnabled,
    DataCacheEnabled,
    BigEndian,
    InterruptsUnmasked,
    DeviceTree,
    ReservedArgument,
    SecondaryCore,
    CounterFrequency,
}

/// Until an EL2 transition is implemented and tested, only an EL1 entry is
/// admitted. Architectural CurrentEL encoding is retained, not a guessed level.
pub fn admit(snapshot: Snapshot) -> Result<(), Refusal> {
    if snapshot.current_el != 4 {
        return Err(Refusal::ExceptionLevel);
    }
    if snapshot.sctlr & 1 != 0 {
        return Err(Refusal::TranslationEnabled);
    }
    if snapshot.sctlr & (1 << 2) != 0 {
        return Err(Refusal::DataCacheEnabled);
    }
    if snapshot.sctlr & ((1 << 25) | (1 << 24)) != 0 {
        return Err(Refusal::BigEndian);
    }
    if snapshot.daif & 0x3c0 != 0x3c0 {
        return Err(Refusal::InterruptsUnmasked);
    }
    if snapshot.dtb_address == 0 || snapshot.dtb_address & 7 != 0 {
        return Err(Refusal::DeviceTree);
    }
    if snapshot.reserved_arguments != [0; 3] {
        return Err(Refusal::ReservedArgument);
    }
    // Ignore MT, U and reserved MPIDR bits; compare all four affinity levels.
    const AFFINITY: u64 = 0x0000_00ff_00ff_ffff;
    if snapshot.mpidr & AFFINITY != snapshot.boot_mpidr & AFFINITY {
        return Err(Refusal::SecondaryCore);
    }
    if snapshot.counter_frequency == 0 || snapshot.counter_frequency > u32::MAX as u64 {
        return Err(Refusal::CounterFrequency);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Snapshot {
        Snapshot {
            current_el: 4,
            sctlr: 0,
            daif: 0x3c0,
            dtb_address: 0x83000000,
            reserved_arguments: [0; 3],
            mpidr: 0x80000000,
            boot_mpidr: 0,
            counter_frequency: 19200000,
        }
    }

    #[test]
    fn valid_el1_entry_allows_instruction_cache_and_non_affinity_bits() {
        let mut state = valid();
        state.sctlr = 1 << 12;
        assert_eq!(admit(state), Ok(()));
    }

    #[test]
    fn unsupported_exception_levels_never_reach_el1_initialization() {
        for current_el in [0, 1, 8, 12, u64::MAX] {
            assert_eq!(
                admit(Snapshot {
                    current_el,
                    ..valid()
                }),
                Err(Refusal::ExceptionLevel)
            );
        }
    }

    #[test]
    fn every_affinity_level_distinguishes_a_secondary_core() {
        for bit in [0, 8, 16, 32] {
            assert_eq!(
                admit(Snapshot {
                    mpidr: 1 << bit,
                    ..valid()
                }),
                Err(Refusal::SecondaryCore)
            );
        }
    }

    #[test]
    fn unsafe_memory_states_bad_dtb_and_unmasked_exceptions_refuse() {
        for (sctlr, expected) in [
            (1, Refusal::TranslationEnabled),
            (4, Refusal::DataCacheEnabled),
            (1 << 25, Refusal::BigEndian),
            (1 << 24, Refusal::BigEndian),
        ] {
            assert_eq!(admit(Snapshot { sctlr, ..valid() }), Err(expected));
        }
        for dtb_address in [0, 0x83000001] {
            assert_eq!(
                admit(Snapshot {
                    dtb_address,
                    ..valid()
                }),
                Err(Refusal::DeviceTree)
            );
        }
        for bit in [6, 7, 8, 9] {
            assert_eq!(
                admit(Snapshot {
                    daif: 0x3c0 & !(1 << bit),
                    ..valid()
                }),
                Err(Refusal::InterruptsUnmasked)
            );
        }
    }

    #[test]
    fn reserved_boot_arguments_and_invalid_counter_frequencies_refuse() {
        for index in 0..3 {
            let mut state = valid();
            state.reserved_arguments[index] = 1;
            assert_eq!(admit(state), Err(Refusal::ReservedArgument));
        }
        for counter_frequency in [0, u32::MAX as u64 + 1] {
            assert_eq!(
                admit(Snapshot {
                    counter_frequency,
                    ..valid()
                }),
                Err(Refusal::CounterFrequency)
            );
        }
    }
}
