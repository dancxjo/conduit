//! Firmware HDMI mode programming from checked EDID timing facts.
use super::mailbox;
use crate::display::edid::PreferredTiming;
#[repr(C, align(64))]
struct Message([u32; 15]);
pub(super) fn set_hdmi_mode(mode: PreferredTiming) -> Result<(), &'static str> {
    if mode.pixel_clock_khz == 0 || mode.pixel_clock_khz > 162_000 {
        return Err("bcm2835-hdmi-pixel-clock-outside-budget");
    }
    let denominator = u64::from(mode.htotal) * u64::from(mode.vtotal);
    if denominator == 0 {
        return Err("bcm2835-hdmi-total-invalid");
    }
    let refresh = (u64::from(mode.pixel_clock_khz) * 1000 + denominator / 2) / denominator;
    let refresh = u16::try_from(refresh).map_err(|_| "bcm2835-hdmi-refresh-invalid")?;
    // Raspberry Pi firmware set_timings ABI, used by the firmware KMS driver:
    // HDMI0 ID=2, no VIC for an EDID custom mode, full-range progressive RGB.
    let mut message = Message([
        60,
        0,
        0x48017,
        36,
        36,
        2,
        mode.pixel_clock_khz,
        mode.width | (u32::from(mode.hsync_start) << 16),
        u32::from(mode.hsync_end) | (u32::from(mode.htotal) << 16),
        mode.height << 16,
        u32::from(mode.vsync_start) | (u32::from(mode.vsync_end) << 16),
        u32::from(mode.vtotal),
        u32::from(refresh),
        u32::from(mode.positive_hsync) | (u32::from(mode.positive_vsync) << 1),
        0,
    ]);
    mailbox::call(&mut message.0)?;
    let response = message.0[4];
    {
        use core::fmt::Write;
        let mut text = crate::sign_format::FixedText::new();
        let _ = writeln!(
            text,
            "CONDUIT_HDMI_TIMING_SET width={} height={} clock_khz={} htotal={} vtotal={} refresh_hz={} response={:#x}",
            mode.width,
            mode.height,
            mode.pixel_clock_khz,
            mode.htotal,
            mode.vtotal,
            refresh,
            response
        );
        super::super::present(text.as_bytes());
    }
    if response & 0x8000_0000 == 0 || response & 0x7fff_ffff > 36 {
        return Err("bcm2835-hdmi-mode-refused");
    }
    Ok(())
}
