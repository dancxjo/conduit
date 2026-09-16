#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("#588 currently promotes only the executable x86_64 ConduitOS backend");

#[cfg(target_os = "none")]
use core::panic::PanicInfo;

#[cfg(target_os = "none")]
use conduitos::{allocation::BOOT_ARENA, arch, boot, sign_format, spore_join, spore_provision};
#[cfg(target_os = "none")]
use core::fmt::Write;

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
            initialize_runtime_arena(&record);
            #[cfg(feature = "native-compositor")]
            graphical_startup::run(record);
            #[cfg(not(feature = "native-compositor"))]
            {
                let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
                let identities = conduitos::identity::derive(
                    entropy,
                    record.timestamp,
                    record.image_physical_start,
                );
                inspect_spore_provision(&record, identities);
                headless_startup::run(record);
            }
        }
        Err(error) => emit_refusal(error.as_str()),
    }
}

#[cfg(target_os = "none")]
fn initialize_runtime_arena(record: &boot::BootRecord) {
    let Some(arena_virtual_start) = record
        .hhdm_offset
        .checked_add(record.runtime_arena.physical_start)
        .and_then(|value| usize::try_from(value).ok())
    else {
        emit_refusal("runtime-arena-address-invalid");
    };
    // SAFETY: normalize_boot selected this non-overlapping admitted runtime
    // arena from the current Limine memory map and the HHDM maps it directly.
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
}

#[cfg(target_os = "none")]
fn inspect_spore_provision(
    _record: &boot::BootRecord,
    identities: conduitos::identity::BootIdentities,
) {
    let Some(region) = boot::spore_module() else {
        emit_refusal("spore-boot-module-missing");
    };
    let provision = match spore_provision::decode(region) {
        Ok(provision) => provision,
        Err(error) => emit_refusal(error.as_str()),
    };
    let mut sign = sign_format::FixedText::new();
    let result = match provision {
        Some(provision) => {
            let spore_id = provision.spore.spore_id.clone();
            let body_id = provision.spore.body_id.clone();
            let invitation_id = provision.invitation_provision.invitation_id.clone();
            let expires_at_millis = provision.invitation_provision.expires_at_millis;
            if let Err(error) = spore_provision::validate_image_binding(
                &provision,
                conduitos::fabrication::EMBEDDED_FABRICATION.target,
                conduitos::fabrication::EMBEDDED_FABRICATION.profile_id,
                conduitos::fabrication::EMBEDDED_FABRICATION.build_id,
            ) {
                emit_refusal(error.as_str());
            }
            let join = match spore_join::encode_native(provision, identities) {
                Ok(join) => join,
                Err(error) => emit_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_SPORE_JOIN ");
            arch::early_write(&join);
            arch::early_write(b"\n");
            writeln!(
                sign,
                "CONDUIT_SPORE_PROVISION {{\"schema\":\"conduit.conduitos/spore-provision@1\",\"status\":\"join-emitted\",\"line_id\":\"conduit-line/serial-text@1\",\"spore_id\":\"{}\",\"body_id\":\"{}\",\"invitation_id\":\"{}\",\"expires_at_millis\":{},\"secret_logged\":false,\"membership_claimed\":false}}",
                spore_id, body_id, invitation_id, expires_at_millis,
            )
        }
        None => writeln!(
            sign,
            "CONDUIT_SPORE_PROVISION {{\"schema\":\"conduit.conduitos/spore-provision@1\",\"status\":\"absent\",\"membership_claimed\":false}}"
        ),
    };
    if result.is_err() {
        emit_refusal("spore-provision-sign-storage-full");
    }
    arch::early_write(sign.as_bytes());
}

#[cfg(all(target_os = "none", feature = "conduitos-isolation-proof"))]
fn run_isolation_proof(record: &boot::BootRecord) {
    let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
    let identities =
        conduitos::identity::derive(entropy, record.timestamp, record.image_physical_start);
    arch::run_isolation_proof(record, identities.host, identities.boot);
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
