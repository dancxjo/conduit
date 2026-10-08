//! Supplemental emulator evidence through real checked Source and the product kernel.
use crate::{arch, boot, identity, offer, ordinary_plan, text_composition};

mod gates;
mod morse;
mod retained_text;
mod timer_runtime;
mod timer;

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
    let mut prepared = match ordinary_plan::prepare_protected(&identities, &offer, make.build_id) {
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
    let plan = prepared.plan.clone();
    drop(prepared);
    gates::run(&plan, &offer);
    morse::run(&plan, &offer);
    timer_runtime::run(&offer);
    keymap_entries();
    retained_text::run(&plan, &offer);
    hostile_entries();
    timer::run();
    arch::early_write(b"CONDUIT_ORDINARY_DOMAIN_SIGN {\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"architecture\":\"x86_64\",\"privilege\":\"ring3\",\"ordinary_source\":true,\"protected_computation\":true,\"effect_capability_gates\":true,\"dma_isolation\":false,\"driver_isolation\":false,\"bounded\":true}\n");
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
        (17, 0, DomainFault::WorkExhausted),
        (7, 0, DomainFault::InvalidInstruction),
        (8, 0, DomainFault::InvalidInstruction),
        (9, 0, DomainFault::PrivilegedOperation),
        (10, 0, DomainFault::InvalidInstruction),
        (11, 0, DomainFault::PrivilegedOperation),
        (12, 0, DomainFault::InvalidInstruction),
        (14, 0, DomainFault::PrivilegedOperation),
        (2, crate::domain_image::USER_TEXT_START, DomainFault::Memory),
        (3, 0x410000, DomainFault::Memory),
    ] {
        let mut domain = arch::TextDomain::install().unwrap_or_else(|_| refuse("proof-domain"));
        domain.probe(command, target);
        let returned = domain.enter(1);
        // SYSENTER in long mode can raise #UD or #GP depending on the CPU.
        let sysenter_denied =
            command == 9 && returned == Ok(DomainReturn::Fault(DomainFault::InvalidInstruction));
        if returned != Ok(DomainReturn::Fault(expected)) && !sysenter_denied {
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
    arch::early_write(b"CONDUIT_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry mmio ports cli loop direction-flag fp syscall sysenter divide breakpoint single-step rdtsc code-write data-execute\n");
}

fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_ORDINARY_DOMAIN_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    arch::deterministic_exit(false)
}

fn keymap_entries() {
    use crate::protected_region::{DomainBackend, DomainReturn};
    use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
    let mut domain = arch::TextDomain::install().unwrap_or_else(|_| refuse("keymap-install"));
    if domain.keymap_input(&[]).is_ok() {
        refuse("keymap-uninitialized-admitted");
    }
    domain
        .initialize_keymap()
        .unwrap_or_else(|_| refuse("keymap-initialize"));
    if domain.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("keymap-initialize-return");
    }
    // Compose state must survive separate hardware entries, including release.
    for (usage, transition, modifiers, expected) in [
        (0xe7, KeyTransition::Pressed, KeyModifiers::RIGHT_GUI, ""),
        (0xe7, KeyTransition::Released, KeyModifiers::NONE, ""),
        (0x34, KeyTransition::Pressed, KeyModifiers::NONE, ""),
        (0x08, KeyTransition::Pressed, KeyModifiers::NONE, "é"),
    ] {
        let event =
            KeyEvent::new(usage, transition, modifiers).unwrap_or_else(|_| refuse("keymap-event"));
        domain
            .keymap_input(&event.encode())
            .unwrap_or_else(|_| refuse("keymap-input"));
        if domain.enter(1) != Ok(DomainReturn::Yielded) {
            refuse("keymap-return");
        }
        let mut output = [0; 256];
        let length = domain
            .output(&mut output)
            .unwrap_or_else(|_| refuse("keymap-output"));
        if &output[..length] != expected.as_bytes() {
            refuse("keymap-compose-state");
        }
    }
    domain
        .input("é".as_bytes())
        .unwrap_or_else(|_| refuse("keymap-upper-input"));
    if domain.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("keymap-upper-return");
    }
    let mut output = [0; 256];
    let length = domain
        .output(&mut output)
        .unwrap_or_else(|_| refuse("keymap-upper-output"));
    if &output[..length] != "É".as_bytes() {
        refuse("keymap-upper-value");
    }
    arch::early_write(
        b"CONDUIT_DOMAIN_KEYMAP retained-compose canonical-sdk protected-uppercase\n",
    );
    let event = KeyEvent::new(0x16, KeyTransition::Pressed, KeyModifiers::RIGHT_ALT)
        .unwrap_or_else(|_| refuse("chain-event"));
    domain
        .keymap_chain_input(&event.encode())
        .unwrap_or_else(|_| refuse("chain-input"));
    let entries = domain.cost().entries;
    if domain.enter(1) != Ok(DomainReturn::Yielded) || domain.cost().entries != entries + 1 {
        refuse("chain-extra-entry");
    }
    let mut intermediate = [0; 4];
    let intermediate_length = domain
        .intermediate(&mut intermediate)
        .unwrap_or_else(|_| refuse("chain-intermediate"));
    let length = domain
        .output(&mut output)
        .unwrap_or_else(|_| refuse("chain-output"));
    if &intermediate[..intermediate_length] != "ß".as_bytes() || &output[..length] != b"SS" {
        refuse("chain-unicode-expansion");
    }
    arch::early_write(b"CONDUIT_DOMAIN_CHAIN one-entry unicode-expansion\n");
}
