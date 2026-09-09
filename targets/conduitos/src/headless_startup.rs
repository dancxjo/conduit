//! Fabricated headless boot without an invented resident workload or offer.
use conduitos::{arch, boot, fabrication, identity, sign_format};
use core::fmt::Write;

pub fn run(record: boot::BootRecord) -> ! {
    let image = &fabrication::EMBEDDED_FABRICATION;
    if image.includes(fabrication::IMPL_NATIVE_PRESENTER)
        || image.facilities != 0
        || image.presenters != 0
        || image.resources != 0
        || image.bases != 0
        || image.drivers != 0
        || image.presentation_surface_slots != 0
        || image.presentation_surface_bytes != 0
        || image.proof_instrumentation != 0
    {
        crate::emit_machine_refusal("headless-startup-inventory-unsupported");
    }
    let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
    let ids = identity::derive(entropy, record.timestamp, record.image_physical_start);
    let mut sign = sign_format::FixedText::new();
    // Compiled implementations are not initialized capabilities. The profile
    // supplies no resident workload and this entry initializes no providers.
    // Keep that absence explicit rather than invoking the graphical demo.
    if writeln!(sign,
        "CONDUIT_HEADLESS_STARTUP {{\"schema\":\"conduit.conduitos/headless-startup@1\",\"status\":\"unsupported\",\"reason\":\"headless-workload-entry-unavailable\",\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_binding\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"compiled_implementations\":{},\"initialized_capabilities\":0,\"offer_generation\":null,\"body_id\":null,\"plan_id\":null,\"active_play_id\":null,\"presenters\":0,\"facilities\":0,\"presentation_surface_slots\":0,\"presentation_surface_bytes\":0,\"runtime_arena_bytes\":{},\"runtime_arena_ceiling\":{},\"allocated_bytes\":{},\"bounded\":true}}",
        image.profile_id, image.build_id, image.image_binding,
        Hex(&ids.host), Hex(&ids.boot), image.implementations,
        record.runtime_arena.length, image.runtime_arena_ceiling, conduitos::allocation::BOOT_ARENA.used(),
    ).is_err() {
        crate::emit_refusal("headless-startup-sign-storage-full");
    }
    arch::early_write(sign.as_bytes());
    arch::deterministic_exit(false)
}

struct Hex<'a>(&'a [u8; 32]);
impl core::fmt::Display for Hex<'_> {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(output, "{byte:02x}")?;
        }
        Ok(())
    }
}
