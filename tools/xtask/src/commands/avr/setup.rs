//! AVR setup contract shared by full xtask and dependency-light dispatch.
use super::{
    avr_toolchain::{avr_gcc_bin, provision, verify_cores},
    rust_toolchain::provision_rust,
};
use std::path::Path;
pub(super) const AVR_HAL_REVISION: &str = "0be252f2a899dbd687a26f8561048ce61854eaae";
pub(super) const FIRMWARE: &str = "targets/avr/firmware/promicro-host";

pub(super) fn check(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    provision_rust()?;
    let cli = provision(root)?;
    verify_cores(&cli, root)?;
    let gcc = avr_gcc_bin(root).join("avr-gcc");
    if !gcc.is_file() {
        return Err(format!("pinned AVR GCC is absent at {}", gcc.display()).into());
    }
    if !root.join(FIRMWARE).join("Cargo.lock").is_file() {
        return Err("Rust AVR firmware lockfile is absent".into());
    }
    Ok(())
}
