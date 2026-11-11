//! Product composition of the shared firmware Face and observed B+ scanout.
use conduitos::{
    arch, cryptographic_entropy::CryptographicEntropyBase, firmware_face::FirmwareFace,
    identity::BootIdentities, make::EMBEDDED_MAKE, offer::HostOffer,
};
#[cfg(feature = "native-startup-chime")]
mod startup;
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

/// The product Root retains native input custody through all refusal paths.
/// USB plots establish port power before scanout samples a USB-powered panel's
/// EDID. Pointer delivery starts only after the ordinary shared Mask is ready.
pub(super) fn run_product(ids: &BootIdentities, offer: &HostOffer<'_>) -> ! {
    use conduitos::raspberry_pi_usb_input::RaspberryPiUsbInput;
    use core::fmt::Write;
    arch::present(b"CONDUIT_BOOT_STAGE usb-before-display\n");
    let mut input = match RaspberryPiUsbInput::prepare(ids) {
        Ok(input) => Some(input),
        Err(reason) => {
            arch::present(b"CONDUIT_USB_INPUT_UNAVAILABLE ");
            arch::present(reason.as_bytes());
            arch::present(b"\n");
            None
        }
    };
    #[cfg(feature = "native-startup-chime")]
    let mut _audio = None;
    let mut graphic = None;
    let mut graphics_prepared = false;
    let mut started = false;
    let mut refused = false;
    loop {
        if !graphics_prepared && (refused || input.as_ref().is_none_or(RaspberryPiUsbInput::ready))
        {
            #[cfg(feature = "native-startup-chime")]
            {
                _audio = startup::play(ids, offer);
            }
            graphic = prepare_graphics(ids, offer);
            graphics_prepared = true;
            arch::present(if cfg!(feature = "aarch64-rpi-3b-plus") {
                b"CONDUIT_BOOT_STAGE rpi3-product-ready\n"
            } else {
                b"CONDUIT_BOOT_STAGE armv6-product-ready\n"
            });
        }
        if !refused {
            if let Some(input) = input.as_mut() {
                let outcome = (|| {
                    if input.ready() && !started {
                        if let Some(graphic) = graphic.as_mut() {
                            // SAFETY: the actual selected endpoint owner is
                            // retained above, and loss retires delivery below.
                            unsafe { graphic.admit_pointer() }?;
                        }
                        input.start_pointer()?;
                        started = true;
                    }
                    if let Some(sample) = input.step()? {
                        if let Some(graphic) = graphic.as_mut() {
                            graphic.observe_pointer(sample)?;
                        }
                        let mut text = conduitos::sign_format::FixedText::new();
                        let _ = writeln!(
                            &mut text,
                            "CONDUIT_POINTER_SAMPLE sequence={} x={} y={} primary={} coalesced={} dropped={}",
                            sample.sequence,
                            sample.position_x,
                            sample.position_y,
                            sample.primary_pressed,
                            sample.coalesced,
                            sample.dropped
                        );
                        arch::present(text.as_bytes());
                    }
                    Ok::<_, &'static str>(())
                })();
                if let Err(reason) = outcome {
                    refused = true;
                    if let Some(graphic) = graphic.as_mut() {
                        let _ = graphic.retire_pointer();
                    }
                    arch::present(b"CONDUIT_POINTER_UNAVAILABLE ");
                    arch::present(reason.as_bytes());
                    arch::present(b"\n");
                }
            }
        }
        if refused || input.as_ref().is_none_or(RaspberryPiUsbInput::needs_tick) {
            // The native Root owns this one-shot machine wake path. Protocol
            // waits themselves complete only through the admitted clock Play.
            arch::timer_arm();
            arch::enable_interrupts();
            arch::interruptible_idle();
            arch::disable_interrupts();
            while arch::pop_interrupt().is_some() {}
        }
    }
}
