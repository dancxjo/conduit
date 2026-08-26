use conduit_host_browser_fabrication::BrowserFabricationPackage;
use conduit_host_conduitos_fabrication::ConduitOsFabricationPackage;
use conduit_host_esp32_fabrication::Esp32FabricationPackage;
use conduit_host_fabrication::{FabricationCatalog, FabricationPackageSet};
use conduit_host_hosted::HostedFabricationPackage;
use conduit_host_raspberry_pi::RaspberryPiFabricationPackage;
use conduit_host_rp2040::Rp2040FabricationPackage;
use conduit_linear_framebuffer_fabrication::LinearFramebufferFabricationExtension;
use conduit_rp2040_pio_audio_extension::Rp2040PioAudioExtension;

/// The finite package environment explicitly chosen by this repository's tooling.
pub fn package_set() -> FabricationPackageSet {
    FabricationPackageSet::compose(&[
        &HostedFabricationPackage,
        &BrowserFabricationPackage,
        &ConduitOsFabricationPackage,
        &Esp32FabricationPackage,
        &Rp2040FabricationPackage,
        &RaspberryPiFabricationPackage,
        &LinearFramebufferFabricationExtension,
        &Rp2040PioAudioExtension,
    ])
    .expect("workspace fabrication package composition is valid")
}

pub fn catalog() -> FabricationCatalog {
    FabricationCatalog::canonical().with_packages(&package_set())
}
