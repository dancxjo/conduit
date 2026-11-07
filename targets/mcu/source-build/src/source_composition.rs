//! One Source-specialized three-fragment MCU composition, compiled for each BSP.
//! This is not an arbitrary-Plot executor or another scheduler: generated exact
//! Source layouts select the sampler, semantic Mask and native output kernels.
//! Board-local adapters retain actual physical owners, timers and transport.
use std::{fs, path::Path};

pub fn emit_to(out: &Path) -> std::io::Result<()> {
    for (name, source) in [
        ("runtime", include_str!("source_composition/runtime.rs")),
        ("play", include_str!("source_composition/play.rs")),
        (
            "play_service",
            include_str!("source_composition/play_service.rs"),
        ),
        ("play_run", include_str!("source_composition/play_run.rs")),
    ] {
        fs::write(out.join(format!("source_{name}.rs")), source)?;
    }
    Ok(())
}
