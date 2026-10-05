#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("#588 currently promotes only the executable x86_64 ConduitOS backend");

#[cfg(target_os = "none")]
use core::panic::PanicInfo;

#[cfg(target_os = "none")]
use conduitos::{allocation::BOOT_ARENA, arch, boot, sign_format};
#[cfg(all(target_os = "none", not(feature = "virtio-net-proof")))]
use conduitos::{spore_join, spore_provision};
#[cfg(all(target_os = "none", not(feature = "virtio-net-proof")))]
use core::fmt::Write;

#[cfg(all(target_os = "none", not(feature = "virtio-net-proof")))]
struct InspectedSpore {
    pending: spore_join::PendingNativeJoin,
    routed_request: Option<conduit_body::RoutedAdmissionRequest>,
}

#[cfg(all(
    target_os = "none",
    feature = "native-compositor",
    not(feature = "virtio-net-proof")
))]
mod graphical_startup;
#[cfg(all(
    target_os = "none",
    not(feature = "native-compositor"),
    not(feature = "virtio-net-proof")
))]
mod headless_startup;
#[cfg(all(
    target_os = "none",
    feature = "native-compositor",
    not(feature = "virtio-net-proof")
))]
mod scripted_startup;

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
extern "C" fn conduitos_start() -> ! {
    match boot::normalize_boot_with_arena_bytes(
        conduitos::make::EMBEDDED_MAKE.runtime_arena_ceiling,
    ) {
        Ok(record) => {
            #[cfg(feature = "emergency-halt-proof")]
            {
                let _ = record;
                arch::early_write(b"CONDUIT_EMERGENCY_HALT_SIGN {\"schema\":\"conduit.conduitos/emergency-halt@1\",\"status\":\"requested\",\"proof_class\":\"freestanding-emulator\",\"authority\":\"proof-appliance\",\"operation\":\"conduitos.machine/halt@1\"}\n");
                arch::emergency_halt();
            }
            #[cfg(not(feature = "emergency-halt-proof"))]
            {
                #[cfg(feature = "virtio-net-proof")]
                run_virtio_net_proof(&record);
                #[cfg(not(feature = "virtio-net-proof"))]
                {
                    #[cfg(feature = "conduitos-isolation-proof")]
                    run_isolation_proof(&record);
                    let make = &conduitos::make::EMBEDDED_MAKE;
                    if let Err(error) = make.validate(record.runtime_arena.length) {
                        emit_machine_refusal(error.as_str());
                    }
                    initialize_runtime_arena(&record);
                    // SAFETY: ordinary native startup is the sole privileged Root.
                    // Protocol modules are local administrator boot configuration;
                    // that administrator must separately approve firmware handoff
                    // and electrical attachment before installing this profile.
                    // Admission checks actual hardware and permanently reserves it.
                    if let Err(reason) =
                        unsafe { conduitos::protocol_boot::run_if_selected(&record) }
                    {
                        emit_machine_refusal(reason);
                    }
                    #[cfg(feature = "native-compositor")]
                    graphical_startup::run(record);
                    #[cfg(not(feature = "native-compositor"))]
                    {
                        let entropy =
                            arch::boot_entropy(record.timestamp, record.image_physical_start);
                        let identities = conduitos::identity::derive(
                            entropy,
                            record.timestamp,
                            record.image_physical_start,
                        );
                        inspect_spore_provision(&record, None);
                        headless_startup::run(record);
                    }
                }
            }
        }
        Err(error) => emit_refusal(error.as_str()),
    }
}

#[cfg(all(target_os = "none", feature = "virtio-net-proof"))]
fn run_virtio_net_proof(record: &boot::BootRecord) -> ! {
    let make = &conduitos::make::EMBEDDED_MAKE;
    if let Err(error) = make.validate(record.runtime_arena.length) {
        emit_machine_refusal(error.as_str());
    }
    initialize_runtime_arena(record);
    let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
    let identities =
        conduitos::identity::derive(entropy, record.timestamp, record.image_physical_start);
    conduitos::virtio_net_proof::run(record, identities)
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

#[cfg(all(target_os = "none", not(feature = "virtio-net-proof")))]
fn inspect_spore_provision(
    _record: &boot::BootRecord,
    advertisement: Option<&conduit_core::HostAdvertisement>,
) -> Option<InspectedSpore> {
    let Some(region) = boot::spore_module() else {
        emit_refusal("spore-boot-module-missing");
    };
    let provision = match spore_provision::decode(region) {
        Ok(provision) => provision,
        Err(error) => emit_refusal(error.as_str()),
    };
    let mut sign = sign_format::FixedText::new();
    let (result, pending) = match provision {
        Some(provision) => {
            let spore_id = provision.spore.spore_id.clone();
            let body_id = provision.spore.body_id.clone();
            let invitation_id = provision.invitation_provision.invitation_id.clone();
            let pending = spore_join::PendingNativeJoin::from_provision(&provision);
            let expires_at_millis = provision.invitation_provision.expires_at_millis;
            if let Err(error) = spore_provision::validate_image_binding(
                &provision,
                conduitos::make::EMBEDDED_MAKE.target,
                conduitos::make::EMBEDDED_MAKE.profile_id,
                conduitos::make::EMBEDDED_MAKE.build_id,
            ) {
                emit_refusal(error.as_str());
            }
            let Some(advertisement) = advertisement else {
                emit_refusal("spore-join-awaits-native-surface");
            };
            let join = match spore_join::prepare_native(provision, advertisement) {
                Ok(join) => join,
                Err(error) => emit_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_SPORE_JOIN ");
            arch::early_write(&join.serial_observation);
            arch::early_write(b"\n");
            let result = writeln!(
                sign,
                "CONDUIT_SPORE_PROVISION {{\"schema\":\"conduit.conduitos/spore-provision@1\",\"status\":\"join-emitted\",\"line_id\":\"conduit-line/serial-text@1\",\"spore_id\":\"{}\",\"body_id\":\"{}\",\"invitation_id\":\"{}\",\"expires_at_millis\":{},\"secret_logged\":false,\"membership_claimed\":false}}",
                spore_id, body_id, invitation_id, expires_at_millis,
            );
            (
                result,
                Some(InspectedSpore {
                    pending,
                    routed_request: join.routed_request,
                }),
            )
        }
        None => (
            writeln!(
                sign,
                "CONDUIT_SPORE_PROVISION {{\"schema\":\"conduit.conduitos/spore-provision@1\",\"status\":\"absent\",\"membership_claimed\":false}}"
            ),
            None,
        ),
    };
    if result.is_err() {
        emit_refusal("spore-provision-sign-storage-full");
    }
    arch::early_write(sign.as_bytes());
    pending
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
fn panic(info: &PanicInfo<'_>) -> ! {
    use core::fmt::Write;
    let mut diagnostic = sign_format::FixedText::new();
    if let Some(location) = info.location() {
        let _ = writeln!(
            diagnostic,
            "CONDUIT_PANIC_LOCATION {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }
    let _ = writeln!(diagnostic, "CONDUIT_PANIC_DETAIL {}", info.message());
    let _ = writeln!(
        diagnostic,
        "CONDUIT_PANIC_ARENA live={} capacity={}",
        BOOT_ARENA.live_bytes(),
        BOOT_ARENA.capacity()
    );
    if let Ok(sign) = sign_format::refused("panic") {
        let _ = diagnostic.write_str(core::str::from_utf8(sign.as_bytes()).unwrap_or(""));
    }
    arch::early_write(diagnostic.as_bytes());
    arch::deterministic_exit(false)
}
