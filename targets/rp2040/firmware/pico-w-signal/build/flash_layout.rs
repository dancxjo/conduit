//! Exact linker custody and identity for the checked board flash regions.
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

pub fn emit(out: &Path) {
    if env::var("TARGET").unwrap_or_default() == "thumbv6m-none-eabi" {
        fs::write(
            out.join("memory.x"),
            conduit_rp2040_flash_layout::LINKER_MEMORY_X,
        )
        .unwrap();
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
    println!("cargo:rerun-if-changed=../../deployment/browser/flash-layout.mjs");
    println!("cargo:rerun-if-changed=build/flash_layout.rs");
}
pub fn identity() -> String {
    format!(
        "{:x}",
        Sha256::digest(conduit_rp2040_flash_layout::LINKER_MEMORY_X.as_bytes())
    )
}
