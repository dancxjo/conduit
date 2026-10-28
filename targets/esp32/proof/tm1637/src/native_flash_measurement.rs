//! Read-only qualification evidence, never a flash grant or timing admission.
//! Counters are updated after the physical owner's critical section returns.
use core::cell::RefCell;
#[path = "native_flash_timing_policy.rs"]
mod policy;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
pub(crate) use policy::{ReviewedTiming, MAXIMUM_BUDGET_MS};
#[derive(Clone, Copy, Default)]
struct Totals {
    erases: u32,
    erase_max_us: u32,
    programs: u32,
    program_max_us: u32,
    program_max_bytes: u32,
    failures: u32,
}
static TOTALS: Mutex<CriticalSectionRawMutex, RefCell<Totals>> = Mutex::new(RefCell::new(Totals {
    erases: 0,
    erase_max_us: 0,
    programs: 0,
    program_max_us: 0,
    program_max_bytes: 0,
    failures: 0,
}));
fn duration(started: embassy_time::Instant) -> u32 {
    let elapsed = embassy_time::Instant::now()
        .as_micros()
        .saturating_sub(started.as_micros());
    elapsed.min(u64::from(u32::MAX)) as u32
}
pub(super) fn erase(started: embassy_time::Instant, failed: bool) {
    let elapsed = duration(started);
    TOTALS.lock(|cell| {
        let mut totals = cell.borrow_mut();
        totals.erases = totals.erases.saturating_add(1);
        totals.erase_max_us = totals.erase_max_us.max(elapsed);
        totals.failures = totals.failures.saturating_add(u32::from(failed));
    });
}
pub(super) fn program(started: embassy_time::Instant, bytes: usize, failed: bool) {
    let elapsed = duration(started);
    TOTALS.lock(|cell| {
        let mut totals = cell.borrow_mut();
        totals.programs = totals.programs.saturating_add(1);
        totals.program_max_us = totals.program_max_us.max(elapsed);
        totals.program_max_bytes = totals
            .program_max_bytes
            .max(bytes.min(u32::MAX as usize) as u32);
        totals.failures = totals.failures.saturating_add(u32::from(failed));
    });
}
#[derive(serde::Serialize)]
pub(crate) struct Snapshot {
    event: &'static str,
    erase_calls: u32,
    maximum_erase_us: u32,
    marker_program_calls: u32,
    maximum_marker_program_us: u32,
    maximum_marker_program_bytes: u32,
    failed_operations: u32,
    timing_qualified: bool,
}
pub(crate) fn snapshot() -> Snapshot {
    let totals = TOTALS.lock(|cell| *cell.borrow());
    Snapshot {
        event: "observed-flash-measurement",
        erase_calls: totals.erases,
        maximum_erase_us: totals.erase_max_us,
        marker_program_calls: totals.programs,
        maximum_marker_program_us: totals.program_max_us,
        maximum_marker_program_bytes: totals.program_max_bytes,
        failed_operations: totals.failures,
        timing_qualified: false,
    }
}
// Espressif esp_rom_spiflash.h v5.3.2 defines these six uint32_t fields.
// esp-rom-sys 0.1.4's ESP32 ROM linker script supplies g_rom_flashchip.
// https://github.com/espressif/esp-idf/blob/v5.3.2/components/esp_rom/include/esp_rom_spiflash.h
#[derive(Clone, Copy)]
#[repr(C)]
struct RomChip {
    device_id: u32,
    chip_size: u32,
    block_size: u32,
    sector_size: u32,
    page_size: u32,
    status_mask: u32,
}
const _: () = assert!(core::mem::size_of::<RomChip>() == 24);
unsafe extern "C" {
    #[link_name = "g_rom_flashchip"]
    static mut ROM_FLASH_CHIP: RomChip;
}
/// Called only at bootstrap with actual exclusive flash custody, before radio
/// tasks or any mutable region is published. This reads ROM's initialized RAM
/// descriptor, not a received identity and not a fresh SPI identification command.
pub(super) fn report_rom_profile() -> Option<ReviewedTiming> {
    // SAFETY: this native image targets ESP32, whose pinned ROM linker script
    // provides the aligned 24-byte descriptor above. The bootloader initialized
    // it before entry. No other driver can reconfigure flash during this call;
    // raw volatile copy creates no shared reference to the mutable C descriptor.
    let chip = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(ROM_FLASH_CHIP)) };
    let reviewed = policy::review(
        chip.device_id,
        chip.chip_size,
        chip.block_size,
        chip.sector_size,
        chip.page_size,
        chip.status_mask,
    );
    esp_println::println!("{{\"event\":\"native-flash-rom-profile\",\"rom_device_id\":{},\"rom_chip_size\":{},\"rom_block_size\":{},\"rom_sector_size\":{},\"rom_page_size\":{},\"rom_status_mask\":{},\"timing_qualified\":false}}",chip.device_id,chip.chip_size,chip.block_size,chip.sector_size,chip.page_size,chip.status_mask);
    reviewed
}
