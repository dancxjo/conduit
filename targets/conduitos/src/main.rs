#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("#588 currently promotes only the executable x86_64 ConduitOS backend");

#[cfg(target_os = "none")]
use core::panic::PanicInfo;

#[cfg(target_os = "none")]
use conduitos::{arch, boot, sign_format};

#[cfg(all(target_os = "none", feature = "native-compositor"))]
mod graphical_startup;
#[cfg(all(target_os = "none", not(feature = "native-compositor")))]
mod headless_startup;
#[cfg(all(target_os = "none", feature = "native-compositor"))]
mod scripted_startup;

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
extern "C" fn conduitos_start() -> ! {
    match boot::normalize_boot() {
        Ok(record) => {
            #[cfg(feature = "conduitos-isolation-proof")]
            run_isolation_proof(&record);
            let fabrication = &conduitos::fabrication::EMBEDDED_FABRICATION;
            if let Err(error) = fabrication.validate(record.runtime_arena.length) {
                emit_machine_refusal(error.as_str());
            }
            #[cfg(feature = "native-compositor")]
            graphical_startup::run(record);
            #[cfg(not(feature = "native-compositor"))]
            headless_startup::run(record);
        }
        Err(error) => emit_refusal(error.as_str()),
    }
}

#[cfg(all(target_os = "none", feature = "conduitos-isolation-proof"))]
fn run_isolation_proof(record: &boot::BootRecord) {
    let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
    let identities =
        conduitos::identity::derive(entropy, record.timestamp, record.image_physical_start);
    arch::run_isolation_proof(record.hhdm_offset, identities.host, identities.boot);
}

#[cfg(target_os = "none")]
fn emit_machine_refusal(reason: &str) -> ! {
    if let Ok(sign) = sign_format::machine_refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[cfg(not(target_os = "none"))]
fn main() {}

#[cfg(target_os = "none")]
fn emit_refusal(reason: &str) -> ! {
    if let Ok(sign) = sign_format::refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[panic_handler]
#[cfg(target_os = "none")]
fn panic(_info: &PanicInfo<'_>) -> ! {
    emit_refusal("panic")
}
