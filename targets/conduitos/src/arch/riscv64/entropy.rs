//! Supervisor-only Zkr seed acquisition with bounded SHA-256 conditioning.
use crate::cryptographic_entropy::{CryptographicEntropySource, EntropyProvider, EntropyRefusal};

#[path = "seed_conditioner.rs"]
mod conditioner;

pub struct SeedEntropy {
    provider: EntropyProvider,
}
impl SeedEntropy {
    pub fn detect(provider_generation: u64) -> Result<Self, EntropyRefusal> {
        if provider_generation == 0 {
            return Err(EntropyRefusal::InvalidProvider);
        }
        let sample = read()?;
        if (sample >> 30) & 3 == 3 {
            return Err(EntropyRefusal::SourceFailure);
        }
        Ok(Self {
            provider: EntropyProvider {
                base_id: "conduitos/base/cryptographic-entropy/zkr-sha256",
                provider_instance_id: "conduitos/riscv64/hart-0/zkr-sha256",
                provider_generation,
            },
        })
    }
}
impl CryptographicEntropySource for SeedEntropy {
    fn provider(&self) -> EntropyProvider {
        self.provider
    }
    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        conditioner::fill(output, read)
    }
}

#[repr(C)]
struct Sample {
    value: u64,
    unavailable: u64,
}
unsafe extern "C" {
    fn conduitos_riscv64_read_seed() -> Sample;
}
fn read() -> Result<u64, EntropyRefusal> {
    let sample = unsafe { conduitos_riscv64_read_seed() };
    if sample.unavailable != 0 {
        Err(EntropyRefusal::Unavailable)
    } else {
        Ok(sample.value)
    }
}

// No feature bit accessible to S-mode authorizes this CSR. Probe exactly one
// instruction with interrupts masked, then restore all touched trap CSRs.
// An unrelated synchronous exception is never mistaken for missing entropy.
core::arch::global_asm!(
    r#"
.section .text.conduitos_riscv64_seed,"ax",@progbits
.balign 4
.global conduitos_riscv64_read_seed
conduitos_riscv64_read_seed:
    csrrci t1, sstatus, 2
    csrr t0, stvec
    csrr t2, sepc
    csrr t3, scause
    csrr t4, stval
    la t5, conduitos_riscv64_seed_trap
    csrw stvec, t5
    li a1, 0
conduitos_riscv64_seed_instruction:
    csrrw a0, 0x015, zero
conduitos_riscv64_seed_resume:
    csrw stvec, t0
    csrw sepc, t2
    csrw scause, t3
    csrw stval, t4
    csrw sstatus, t1
    ret
.balign 4
conduitos_riscv64_seed_trap:
    csrr t5, scause
    li t6, 2
    bne t5, t6, conduitos_riscv64_seed_root_fault
    csrr t5, sepc
    la t6, conduitos_riscv64_seed_instruction
    bne t5, t6, conduitos_riscv64_seed_root_fault
    li a0, 0
    li a1, 1
    la t5, conduitos_riscv64_seed_resume
    csrw sepc, t5
    sret
conduitos_riscv64_seed_root_fault:
    csrci sstatus, 2
1:  wfi
    j 1b
"#
);
