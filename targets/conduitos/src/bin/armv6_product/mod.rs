//! Product composition of the shared firmware Face and observed B+ scanout.
use conduitos::{
    arch, cryptographic_entropy::CryptographicEntropyBase, firmware_face::FirmwareFace,
    identity::BootIdentities, make::EMBEDDED_MAKE, offer::HostOffer,
};
pub(super) fn prepare_graphics(
    ids: &BootIdentities,
    offer: &HostOffer<'_>,
) -> Option<FirmwareFace<arch::FirmwareScanout>> {
    let result = (|| {
        let display = arch::FirmwareScanout::prepare()?;
        let (physical, bytes) = display.physical_range();
        let (firmware, preferred) = display.mode_observation();
        let (clock, timing_set) = display.timing_set_observation();
        let evidence = serde_json::to_vec(&serde_json::json!({
            "firmware_width":firmware.0,"firmware_height":firmware.1,
            "edid_preferred_width":preferred.map(|mode| mode.0),
            "edid_preferred_height":preferred.map(|mode| mode.1),
            "edid_pixel_clock_khz":clock,
            "hdmi_timing_set_acknowledged":timing_set.map(|result|result.is_ok()),
            "hdmi_timing_set_refusal":timing_set.and_then(Result::err)
        }))
        .map_err(|_| "display-mode-evidence-encoding")?;
        arch::present(b"CONDUIT_DISPLAY_MODE ");
        arch::present(&evidence);
        arch::present(b"\n");
        arch::present(b"CONDUIT_BOOT_STAGE framebuffer-allocated\n");
        let mut entropy = CryptographicEntropyBase::<_, 1>::admit(arch::HardwareEntropy)
            .map_err(|e| e.as_str())?;
        let graphic = entropy
            .with_secret::<32, _>(|key, _| {
                arch::present(b"CONDUIT_BOOT_STAGE firmware-face-prepare\n");
                FirmwareFace::prepare(display, ids, offer, &EMBEDDED_MAKE, *key)
            })
            .map_err(|e| {
                let (control, status) = arch::HardwareEntropy::readiness();
                let evidence = serde_json::to_vec(&serde_json::json!({
                    "control":control,"warmup_remaining":status & 0x00ff_ffff,
                    "fifo_words":status >> 24,"reason":e.as_str()
                }));
                if let Ok(evidence) = evidence {
                    arch::present(b"CONDUIT_ENTROPY_UNAVAILABLE ");
                    arch::present(&evidence);
                    arch::present(b"\n");
                }
                e.as_str()
            })??;
        let timing = graphic.scanout().hdmi_timing();
        let observation = serde_json::to_vec(&serde_json::json!({
            "pixel_probe_words":graphic.scanout().observed_pixels(),
            "probe_columns":"left,center,right","probe_rows":"top,middle,bottom",
            "hdmi_width":timing.map(|value|value.0),"hdmi_height":timing.map(|value|value.1),
            "hdmi_pixel_clock_khz":timing.map(|value|value.2)
        }))
        .map_err(|_| "framebuffer-observation-encoding")?;
        arch::present(b"CONDUIT_FRAMEBUFFER_OBSERVATION ");
        arch::present(&observation);
        arch::present(b"\n");
        let evidence=serde_json::to_vec(&serde_json::json!({"schema":"conduit.conduitos/bcm2835-framebuffer@1","physical_start":physical,"bytes":bytes,"cache_policy":"normal-noncacheable","identity_mapping":true,"gpu_acceleration_proved":false})).map_err(|_|"framebuffer-evidence-encoding")?;
        arch::present(b"CONDUIT_FRAMEBUFFER_MAPPING ");
        arch::present(&evidence);
        arch::present(b"\n");
        arch::present(conduitos::firmware_face::SIGN_PREFIX);
        arch::present(graphic.evidence());
        arch::present(b"\n");
        Ok::<_, &'static str>(graphic)
    })();
    match result {
        Ok(graphic) => Some(graphic),
        Err(reason) => {
            arch::present(b"CONDUIT_DISPLAY_UNAVAILABLE ");
            arch::present(reason.as_bytes());
            arch::present(b"\n");
            None
        }
    }
}
