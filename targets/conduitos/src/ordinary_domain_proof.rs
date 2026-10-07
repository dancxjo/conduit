//! Supplemental emulator evidence through real checked Source and the product kernel.
use crate::{arch, boot, identity, offer, ordinary_plan, text_composition};

pub fn run(record: &boot::BootRecord) -> ! {
    arch::initialize_machine(record, boot::executable_physical_address);
    let identities = identity::derive(
        arch::boot_entropy(record.timestamp, record.image_physical_start),
        record.timestamp,
        record.image_physical_start,
    );
    let make = &crate::make::EMBEDDED_MAKE;
    let offer = offer::HostOffer::new(
        &identities,
        make.build_id,
        offer::CpuFeatures {
            sse2: true,
            rdrand: false,
            invariant_tsc: false,
        },
        record.runtime_arena.length,
    );
    let mut prepared = match ordinary_plan::prepare(&identities, &offer, make.build_id) {
        Ok(prepared) => prepared,
        Err(error) => refuse(error.as_str()),
    };
    let mut observed = text_composition::TextObservations::default();
    let result = text_composition::run_observed(
        &mut prepared.kernel,
        &mut arch::Clock::new(),
        &mut arch::Serial::new(),
        &mut arch::Interrupts::new(),
        &mut arch::Idle::new(),
        &mut observed,
    );
    if let Err(error) = result {
        refuse(error.as_str());
    }
    drop(prepared);
    hostile_entries();
    arch::early_write(b"CONDUIT_ORDINARY_DOMAIN_SIGN {\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"architecture\":\"x86_64\",\"privilege\":\"ring3\",\"ordinary_source\":true,\"protected_computation\":true,\"effect_capability_gates\":false,\"dma_isolation\":false,\"driver_isolation\":false,\"bounded\":true}\n");
    arch::deterministic_exit(true)
}

fn hostile_entries() {
    use crate::protected_region::{DomainBackend, DomainFault, DomainReturn};
    let private = 0x5a5a_5a5a_u64;
    let capabilities = crate::protection_domain::KernelCapabilityTable::new(7)
        .unwrap_or_else(|_| refuse("proof-capability-table"));
    let mut sibling = arch::TextDomain::install().unwrap_or_else(|_| refuse("proof-sibling"));
    sibling
        .input(b"sentinel")
        .unwrap_or_else(|_| refuse("proof-sibling-input"));
    let sibling_address = sibling.private_frame_address();
    let sibling_before = unsafe { (sibling_address as *const u32).read_volatile() };
    for (command, target, expected) in [
        (1, &private as *const u64 as u64, DomainFault::Memory),
        (1, &capabilities as *const _ as u64, DomainFault::Memory),
        (2, sibling_address, DomainFault::Memory),
        (
            3,
            arch::early_write as *const () as u64,
            DomainFault::Memory,
        ),
        (1, 0xfee0_0000, DomainFault::Memory),
        (4, 0, DomainFault::PrivilegedOperation),
        (5, 0, DomainFault::PrivilegedOperation),
        (6, 0, DomainFault::WorkExhausted),
        (7, 0, DomainFault::InvalidInstruction),
        (2, crate::domain_image::USER_TEXT_START, DomainFault::Memory),
        (3, 0x410000, DomainFault::Memory),
    ] {
        let mut domain = arch::TextDomain::install().unwrap_or_else(|_| refuse("proof-domain"));
        domain.probe(command, target);
        if domain.enter(1) != Ok(DomainReturn::Fault(expected)) {
            refuse("hostile-entry-did-not-fault");
        }
        if domain.cost().scheduler_returns != 1 {
            refuse("hostile-entry-did-not-return-to-root");
        }
    }
    if private != 0x5a5a_5a5a
        || unsafe { (sibling_address as *const u32).read_volatile() } != sibling_before
    {
        refuse("private-state-changed");
    }
    arch::early_write(b"CONDUIT_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry mmio ports cli loop fp code-write data-execute\n");
}

fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_ORDINARY_DOMAIN_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    arch::deterministic_exit(false)
}
