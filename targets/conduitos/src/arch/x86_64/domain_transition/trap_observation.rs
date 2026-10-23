//! Single-entry, supervisor-private observation: fourteen words, exactly 112 bytes.
unsafe extern "C" {
    static mut conduitos_trap_observation: [u64; 14];
}

pub(super) fn take() -> [u64; 14] {
    let pointer = core::ptr::addr_of_mut!(conduitos_trap_observation).cast::<u64>();
    // ACTIVE serializes ordinary entry on the proof's single CPU. The caller
    // consumes immediately after assembly returns, while interrupts are masked.
    // The inherited Root mapping is supervisor-only in domain_memory::install.
    unsafe {
        let raw = core::array::from_fn(|index| pointer.add(index).read_volatile());
        pointer.write_volatile(0);
        raw
    }
}
const _: () = assert!(core::mem::size_of::<[u64; 14]>() == 112);

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    #[test]
    fn disabled_observer_retains_exact_81eb_production_assembly_text() {
        // Hash of the complete raw global_asm body at accepted base 81eb177.
        // This deterministic source guard is not an assembled-byte receipt:
        // native verification must also compare disabled .text and relocations.
        let source = include_str!("entry_return.S");
        let mut hash = Sha256::new();
        let mut proof = false;
        let mut definition = false;
        for line in source.split_inclusive('\n') {
            if line.trim() == ".macro envelope_stamp offset" {
                definition = true;
                continue;
            }
            if definition {
                if line.trim() == ".endm" {
                    definition = false;
                }
                continue;
            }
            if line.trim() == ".if {proof_envelope}" {
                assert!(!proof);
                proof = true;
                continue;
            }
            if proof {
                if line.trim() == ".endif" {
                    proof = false;
                }
                continue;
            }
            if line.starts_with("    // Proof-only stamps")
                || line.starts_with("    // preserved by each caller")
            {
                continue;
            }
            hash.update(line.as_bytes());
        }
        assert!(!proof && !definition);
        assert_eq!(
            <[u8; 32]>::from(hash.finalize()),
            [
                0xa4, 0x4d, 0x45, 0x47, 0x0c, 0x42, 0x3e, 0x7c, 0xee, 0x0f, 0x62, 0xed, 0xb8, 0xa7,
                0x40, 0x25, 0x74, 0x5b, 0x7a, 0xde, 0x5e, 0xda, 0x29, 0x0b, 0x9b, 0xc8, 0xa3, 0x0e,
                0xe5, 0xd2, 0xc9, 0x33
            ]
        );
    }
}
