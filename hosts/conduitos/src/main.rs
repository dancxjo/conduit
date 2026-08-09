#![no_std]
#![no_main]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("#588 currently promotes only the executable x86_64 ConduitOS backend");

#[cfg(target_os = "none")]
use core::panic::PanicInfo;

use conduitos::{arch, boot, identity, proof};

#[cfg(target_os = "none")]
const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
#[cfg(not(target_os = "none"))]
const BUILD_ID: &str = "host-check-only";
#[cfg(target_os = "none")]
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");
#[cfg(not(target_os = "none"))]
const IMAGE_ID: &str = "host-check-only";

#[unsafe(no_mangle)]
extern "C" fn conduitos_start() -> ! {
    match boot::normalize_boot() {
        Ok(record) => {
            let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
            let identities =
                identity::derive(entropy, record.timestamp, record.image_physical_start);
            match proof::accepted(&record, &identities, BUILD_ID, IMAGE_ID) {
                Ok(sign) => {
                    arch::early_write(sign.as_bytes());
                    arch::deterministic_exit(true);
                }
                Err(_) => emit_refusal("boot-sign-storage-full"),
            }
        }
        Err(error) => emit_refusal(error.as_str()),
    }
}

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
