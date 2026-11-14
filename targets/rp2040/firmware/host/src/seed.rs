//! RP2040-owned immutable carrier address; shared construction decoder.
pub struct Seed<'a>(conduit_mcu_host_runtime::seed::Seed<'a>);
impl<'a> core::ops::Deref for Seed<'a> {
    type Target = conduit_body::MemberBootstrap<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Seed<'a> {
    pub fn into_shared(self) -> conduit_mcu_host_runtime::seed::Seed<'a> {
        self.0
    }
}
impl Seed<'static> {
#[inline(never)]
    pub fn load(actual_host: &str) -> Option<Self> {
        const REGION: usize = conduit_rp2040_flash_layout::BOOTSTRAP.bytes() as usize;
        // SAFETY: immutable XIP view of the target-reserved sector, no write access.
        let bytes = unsafe {
            core::slice::from_raw_parts(
                conduit_rp2040_flash_layout::BOOTSTRAP.address() as *const u8,
                REGION,
            )
        };
        conduit_mcu_host_runtime::seed::Seed::parse(
            bytes,
            actual_host,
            crate::config::IMAGE_ID,
            crate::config::CONFIGURATION_ID,
        )
        .map(Self)
    }
}
