//! Compose the shared checked firmware Face with the IA-32 machine leaf.
use conduitos::{arch, identity, make::EMBEDDED_MAKE, offer_make::ImageBoundHostOffer};
use core::ops::Range;

type Graphic = conduitos::firmware_face::FirmwareFace<arch::framebuffer::FirmwareScanout>;

/// # Safety
/// Original Multiboot handoff after machine initialization; `owned` covers Root
/// and the retained spore module. Root retains the returned exclusive display.
pub(super) unsafe fn prepare(
    magic: u32,
    information: u32,
    owned: &[Range<u64>],
    identities: &identity::BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
) -> Option<Graphic> {
    if !EMBEDDED_MAKE.includes(conduitos::make::IMPL_NATIVE_PRESENTER) {
        return None;
    }
    let display = unsafe { arch::framebuffer::FirmwareScanout::prepare(magic, information, owned) }
        .unwrap_or_else(|reason| super::refuse(reason));
    let policy = *display.policy();
    let memory = *display.memory();
    let source = arch::DomainEntropy::detect(1).unwrap_or_else(|e| super::refuse(e.as_str()));
    let mut entropy =
        conduitos::cryptographic_entropy::CryptographicEntropyBase::<_, 1>::admit(source)
            .unwrap_or_else(|e| super::refuse(e.as_str()));
    let graphic = entropy
        .with_secret::<32, _>(|key, _| {
            conduitos::firmware_face::FirmwareFace::prepare(
                display,
                identities,
                offer,
                &EMBEDDED_MAKE,
                *key,
            )
        })
        .unwrap_or_else(|e| super::refuse(e.as_str()))
        .unwrap_or_else(|reason| super::refuse(reason));
    let proof = serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.conduitos/ia32-framebuffer-mapping@1",
        "host_id":identity::hex(&identities.host),"boot_id":identity::hex(&identities.boot),
        "base_id":graphic.framebuffer_basis().base_id,
        "physical_start":policy.physical_start,"virtual_start":policy.physical_start,
        "byte_length":policy.byte_length,"cache_line_bytes":policy.cache_line_bytes,
        "mapping":"root-flat-protected-mode-paging-disabled",
        "store_completion":"volatile-word-stores-mfence-clflush-modified-lines-mfence",
        "memory_type_claimed":false,"exclusive_writer":true,
        "memory_map_entries":memory.memory_map_entries,
        "covered_by_reserved_entry":memory.covered_by_reserved_entry,
        "nonreserved_overlap":false,"owned_storage_overlap":false,
        "physical_scanout_proved":false,"gpu_acceleration_proved":false
    }))
    .unwrap_or_else(|_| super::refuse("framebuffer-mapping-evidence-encoding"));
    if proof.len() > 2048 {
        super::refuse("framebuffer-mapping-evidence-bound");
    }
    arch::present(b"CONDUIT_FRAMEBUFFER_MAPPING ");
    arch::present(&proof);
    arch::present(b"\n");
    arch::present(conduitos::firmware_face::SIGN_PREFIX);
    arch::present(graphic.evidence());
    arch::present(b"\n");
    Some(graphic)
}
