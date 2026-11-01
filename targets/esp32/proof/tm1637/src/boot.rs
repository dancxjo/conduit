//! Actual boot observation from initialized radio entropy and eFuse identity.
//! This is neither a Host offer, a Body birth nor an image-attestation receipt.
use core::fmt::Write;
use esp_hal::{
    efuse::{self, InterfaceMacAddress},
    rng::Trng,
};
use heapless::String;

pub struct ObservedBoot {
    host: String<32>,
    boot: String<40>,
}
impl ObservedBoot {
    /// Call after the Wi-Fi controller has enabled the real entropy source.
    /// Failure to obtain a TRNG refuses observation; no pseudorandom fallback.
    pub fn observe() -> Result<Self, esp_hal::rng::TrngError> {
        let rng = Trng::try_new()?;
        let mac = efuse::interface_mac_address(InterfaceMacAddress::Station);
        let mut nonce = [0_u8; 16];
        rng.read(&mut nonce);
        let mut host = String::new();
        host.push_str("esp32/").unwrap();
        for octet in mac.as_bytes() {
            write!(&mut host, "{octet:02x}").unwrap();
        }
        let mut boot = String::new();
        boot.push_str("boot/").unwrap();
        for octet in nonce {
            write!(&mut boot, "{octet:02x}").unwrap();
        }
        Ok(Self { host, boot })
    }
    #[cfg(feature = "native-root")]
    pub fn ids(&self) -> (&str, &str) {
        (&self.host, &self.boot)
    }
    #[cfg(not(feature = "native-root"))]
    pub fn print(&self) {
        esp_println::println!(
            "{{\"event\":\"native-boot-observation\",\"schema\":\"conduit.environment/esp32-boot@1\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"host_offer\":false,\"body_installed\":false,\"image_sha256\":null}}",
            self.host, self.boot
        );
    }
}
