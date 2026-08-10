#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("#588 currently promotes only the executable x86_64 ConduitOS backend");

#[cfg(target_os = "none")]
use core::panic::PanicInfo;

#[cfg(target_os = "none")]
use conduitos::{allocation::BOOT_ARENA, arch, boot, dual_region_plan, identity, proof};

#[cfg(target_os = "none")]
const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
#[cfg(target_os = "none")]
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
extern "C" fn conduitos_start() -> ! {
    match boot::normalize_boot() {
        Ok(record) => {
            let Some(arena_virtual_start) = record
                .hhdm_offset
                .checked_add(record.runtime_arena.physical_start)
                .and_then(|value| usize::try_from(value).ok())
            else {
                emit_refusal("runtime-arena-address-invalid");
            };
            if unsafe {
                BOOT_ARENA.initialize(
                    arena_virtual_start,
                    usize::try_from(record.runtime_arena.length).unwrap_or(0),
                )
            }
            .is_err()
            {
                emit_refusal("runtime-arena-initialization-failed");
            }
            let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
            let identities =
                identity::derive(entropy, record.timestamp, record.image_physical_start);
            match proof::accepted(&record, &identities, BUILD_ID, IMAGE_ID) {
                Ok(sign) => {
                    arch::early_write(sign.as_bytes());
                }
                Err(_) => emit_refusal("boot-sign-storage-full"),
            }
            let offer = conduitos::offer::HostOffer::new(
                &identities,
                BUILD_ID,
                arch::feature_basis(),
                record.runtime_arena.length,
            );
            if let Err(error) = offer.validate() {
                emit_machine_refusal(error.as_str());
            }
            let mut prepared = match dual_region_plan::prepare(&identities, &offer, BUILD_ID) {
                Ok(prepared) => prepared,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let observatory_export = match conduitos::observatory::prepare_export(
                &record,
                &identities,
                &offer,
                &prepared,
                BUILD_ID,
                IMAGE_ID,
            ) {
                Ok(export) => export,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let allocation_before_play = BOOT_ARENA.seal();
            arch::initialize_machine();
            let mut clock = arch::Clock::new();
            let mut timer = arch::Timer::new();
            let mut serial = arch::Serial::new();
            let mut interrupts = arch::Interrupts::new();
            let mut idle = arch::Idle::new();
            match conduitos::dual_region_composition::run(
                &mut prepared.kernel,
                &mut clock,
                &mut timer,
                &mut serial,
                &mut interrupts,
                &mut idle,
            ) {
                Ok(report) => match proof::machine_accepted(
                    &identities,
                    &offer,
                    &report,
                    &prepared,
                    proof::AllocationProof {
                        before_play: allocation_before_play,
                        after_play: BOOT_ARENA.used(),
                        capacity: BOOT_ARENA.capacity(),
                    },
                    BUILD_ID,
                ) {
                    Ok(sign) => {
                        arch::early_write(sign.as_bytes());
                        arch::early_write(conduitos::observatory::EXPORT_PREFIX.as_bytes());
                        arch::early_write(observatory_export.as_bytes());
                        arch::early_write(b"\n");
                        arch::deterministic_exit(true);
                    }
                    Err(_) => emit_machine_refusal("kernel-sign-storage-full"),
                },
                Err(error) => emit_machine_refusal(error.as_str()),
            }
        }
        Err(error) => emit_refusal(error.as_str()),
    }
}

#[cfg(target_os = "none")]
fn emit_machine_refusal(reason: &str) -> ! {
    if let Ok(sign) = proof::machine_refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[cfg(not(target_os = "none"))]
fn main() {}

#[cfg(target_os = "none")]
fn emit_refusal(reason: &str) -> ! {
    if let Ok(sign) = proof::refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[panic_handler]
#[cfg(target_os = "none")]
fn panic(_info: &PanicInfo<'_>) -> ! {
    emit_refusal("panic")
}
