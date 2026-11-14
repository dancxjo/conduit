//! Only the target-owned, independently preflighted immutable carrier is read.
use conduit_mcu_host_runtime::seed::Seed;
#[repr(align(4))]
pub struct Carrier([u32; 1024]);
impl Carrier {
    pub const fn new() -> Self {
        Self([0; 1024])
    }
    /// Boot-only read before native tasks/flash users are started. Callers must
    /// preserve the current device's complete flash and verify this reserved
    /// sector does not overlap retained state before provisioning it.
    pub fn load(&'static mut self, host: &str) -> Option<Seed<'static>> {
        // SAFETY: writable aligned owned 4KiB target buffer; fixed checked
        // 4MiB flash carrier extent, no flash writes or concurrent flash users.
        let result = unsafe {
            esp_hal::rom::spiflash::esp_rom_spiflash_read(
                crate::config::BOOTSTRAP_ADDRESS,
                self.0.as_mut_ptr(),
                4096,
            )
        };
        if result != esp_hal::rom::spiflash::ESP_ROM_SPIFLASH_RESULT_OK {
            return None;
        }
        // SAFETY: immutable bytes of the initialized aligned buffer, retained
        // for the full boot. The resulting borrowed Seed prevents mutation.
        let bytes = unsafe { core::slice::from_raw_parts(self.0.as_ptr().cast::<u8>(), 4096) };
        Seed::parse(
            bytes,
            host,
            crate::config::IMAGE_ID,
            crate::config::CONFIGURATION_ID,
        )
    }
}
