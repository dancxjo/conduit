use conduit_host_avr_make::AvrProMicroMakePackage;
use conduit_host_browser_make::BrowserMakePackage;
use conduit_host_conduitos_make::ConduitOsMakePackage;
use conduit_host_esp32_make::Esp32MakePackage;
pub use conduit_host_hosted::CheckedSourceCheckInitialization;
use conduit_host_hosted::HostedMakePackage;
use conduit_host_make::{MakeCatalog, MakePackageSet};
use conduit_host_orange_pi::OrangePiMakePackage;
use conduit_host_raspberry_pi::RaspberryPiMakePackage;
use conduit_host_rp2040::Rp2040MakePackage;
use conduit_linear_framebuffer_make::LinearFramebufferMakeExtension;
pub use conduit_linear_framebuffer_make::{
    CheckedNativeMaskInitialization, CheckedScanoutInitialization,
};

mod environment;
pub use environment::*;

#[cfg(test)]
mod environment_tests;

/// The finite package environment explicitly chosen by this repository's tooling.
pub fn package_set() -> MakePackageSet {
    MakePackageSet::compose(&[
        &AvrProMicroMakePackage,
        &HostedMakePackage,
        &BrowserMakePackage,
        &ConduitOsMakePackage,
        &Esp32MakePackage,
        &OrangePiMakePackage,
        &Rp2040MakePackage,
        &RaspberryPiMakePackage,
        &LinearFramebufferMakeExtension,
    ])
    .expect("workspace make package composition is valid")
}

pub fn catalog() -> MakeCatalog {
    MakeCatalog::canonical().with_packages(&package_set())
}

#[cfg(test)]
mod graphical_host_tests;

pub use conduit_host_hosted::{CheckedTextFileInitialization, TEXT_FILE_BASE_KIND};

pub use conduit_host_hosted::{CheckedCpuInitialization, CPU_BASE_KIND, CPU_IMPLEMENTATION};
