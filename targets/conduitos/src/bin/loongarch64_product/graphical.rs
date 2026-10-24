//! Compose the shared checked firmware Face with the LoongArch machine leaf.
use super::refuse;
use conduitos::{arch, boot, identity, make::EMBEDDED_MAKE, offer_make::ImageBoundHostOffer};

pub(super) fn prepare(
    record: &boot::BootRecord,
    identities: &identity::BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
) -> Option<conduitos::firmware_face::FirmwareFace<arch::framebuffer::FirmwareScanout>> {
    if !EMBEDDED_MAKE.includes(conduitos::make::IMPL_NATIVE_PRESENTER) {
        return None;
    }
    let display = boot::framebuffer_display().unwrap_or_else(|error| refuse(error.as_str()));
    let display = arch::framebuffer::FirmwareScanout::prepare(display, record)
        .unwrap_or_else(|reason| refuse(reason));
    let mapping = *display.mapping();
    let source = arch::FwCfgEntropy::detect(1).unwrap_or_else(|error| refuse(error.as_str()));
    let mut entropy =
        conduitos::cryptographic_entropy::CryptographicEntropyBase::<_, 1>::admit(source)
            .unwrap_or_else(|error| refuse(error.as_str()));
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
        .unwrap_or_else(|error| refuse(error.as_str()))
        .unwrap_or_else(|reason| refuse(reason));
    let proof = serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.conduitos/loongarch64-framebuffer-mapping@1",
        "host_id":identity::hex(&identities.host),"boot_id":identity::hex(&identities.boot),
        "base_id":graphic.framebuffer_basis().base_id,"mapping":mapping,"cache_policy":"mat-observed-qemu-tcg-shared-ram",
        "store_completion":"volatile-stores-dbar0",
        "exclusive_writer":true,"physical_memory_type_claimed":false,
        "framebuffer_reserved":true,"table_page_custody":"bootloader-reclaimable",
        "physical_scanout_proved":false,"gpu_acceleration_proved":false
    }))
    .unwrap_or_else(|_| refuse("framebuffer-mapping-evidence-encoding"));
    if proof.len() > 2048 {
        refuse("framebuffer-mapping-evidence-bound");
    }
    arch::present(b"CONDUIT_FRAMEBUFFER_MAPPING ");
    arch::present(&proof);
    arch::present(b"\n");
    arch::present(conduitos::firmware_face::SIGN_PREFIX);
    arch::present(graphic.evidence());
    arch::present(b"\n");
    Some(graphic)
}
