//! One Source-specialized three-fragment MCU composition, compiled for each BSP.
//! This is not an arbitrary-Plot executor or another scheduler: generated exact
//! Source layouts select the sampler, semantic Mask and native output kernels.
//! Board-local adapters retain actual physical owners, timers and transport.
use std::{fs, path::Path};

pub fn emit_to(out: &Path) -> std::io::Result<()> {
    for (name, source) in [
        ("runtime", include_str!("source_composition/runtime.rs")),
        (
            "runtime_transactions",
            include_str!("source_composition/runtime_transactions.rs"),
        ),
        (
            "runtime_continuation",
            include_str!("source_composition/runtime_continuation.rs"),
        ),
        (
            "runtime_continuation_factory",
            include_str!("source_composition/runtime_continuation_factory.rs"),
        ),
        (
            "play_continuation_start",
            include_str!("source_composition/play_continuation_start.rs"),
        ),
        (
            "play_initialization",
            include_str!("source_composition/play_initialization.rs"),
        ),
        (
            "play_recording",
            include_str!("source_composition/play_recording.rs"),
        ),
        (
            "play_authority",
            include_str!("source_composition/play_authority.rs"),
        ),
        ("play", include_str!("source_composition/play.rs")),
        (
            "play_continuation",
            include_str!("source_composition/play_continuation.rs"),
        ),
        (
            "play_continuation_native",
            include_str!("source_composition/play_continuation_native.rs"),
        ),
        (
            "play_owners",
            include_str!("source_composition/play_owners.rs"),
        ),
        (
            "play_shapes",
            include_str!("source_composition/play_shapes.rs"),
        ),
        (
            "play_service",
            include_str!("source_composition/play_service.rs"),
        ),
        (
            "play_presentation",
            include_str!("source_composition/play_presentation.rs"),
        ),
        ("play_run", include_str!("source_composition/play_run.rs")),
        (
            "play_observers",
            include_str!("source_composition/play_observers.rs"),
        ),
        (
            "play_inspection",
            include_str!("source_composition/play_inspection.rs"),
        ),
    ] {
        fs::write(out.join(format!("source_{name}.rs")), source)?;
    }
    Ok(())
}
