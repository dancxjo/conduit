//! Observed firmware geometry; this does not admit a graphical presenter.
use conduit_core::HostBaseId;
use conduit_observatory::FramebufferBasis;
use conduitos::{
    boot::{Firmware, Multiboot1FramebufferFacts},
    identity::{self, BootIdentities},
    make::EMBEDDED_MAKE,
};

pub(super) fn basis(
    facts: Option<&Multiboot1FramebufferFacts>,
    identities: &BootIdentities,
    firmware: Firmware,
) -> Option<FramebufferBasis> {
    let basis = facts.map(|facts| FramebufferBasis {
        base_id: HostBaseId::from(identity::hex(&identity::derive_base(
            &identities.boot,
            "conduitos/display/firmware/0",
        ))),
        width: facts.format.width,
        height: facts.format.height,
        pitch_bytes: facts.format.pitch,
        bits_per_pixel: facts.format.bits_per_pixel,
    });
    let framebuffer = facts.zip(basis.as_ref()).map(|(facts, basis)| {
        serde_json::json!({"base_id":basis.base_id,"physical_start":facts.physical_start,
            "byte_length":facts.byte_len,"width":facts.format.width,
            "height":facts.format.height,"pitch_bytes":facts.format.pitch,
            "bits_per_pixel":facts.format.bits_per_pixel,"red_shift":facts.format.red_shift,
            "green_shift":facts.format.green_shift,"blue_shift":facts.format.blue_shift})
    });
    let evidence = serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.conduitos/ia32-framebuffer-observation@1",
        "target":EMBEDDED_MAKE.target,"profile_id":EMBEDDED_MAKE.profile_id,
        "build_id":EMBEDDED_MAKE.build_id,"image_id":EMBEDDED_MAKE.image_binding,
        "host_id":identity::hex(&identities.host),"boot_id":identity::hex(&identities.boot),
        "firmware":firmware.as_str(),"framebuffer":framebuffer,
        "graphical_presenter_selected":false,"graphical_show_proved":false
    }))
    .unwrap_or_else(|_| super::refuse("framebuffer-observation-encoding"));
    if evidence.len() > 2048 {
        super::refuse("framebuffer-observation-bound");
    }
    conduitos::arch::present(b"CONDUIT_IA32_FRAMEBUFFER ");
    conduitos::arch::present(&evidence);
    conduitos::arch::present(b"\n");
    basis
}
