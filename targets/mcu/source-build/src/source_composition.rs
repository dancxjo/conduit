//! One Source-specialized three-fragment MCU composition, compiled for each BSP.
//! This is not an arbitrary-Plot executor or another scheduler: generated exact
//! Source layouts select the sampler, semantic Mask and native output kernels.
//! Board-local adapters retain actual physical owners, timers and transport.
use std::{fs, path::Path};

#[cfg(test)]
#[path = "source_composition/play_continuation_bindings.rs"]
mod continuation_binding_tests;

pub fn emit_to(out: &Path) -> std::io::Result<()> {
    for (name, source) in [
        ("runtime", include_str!("source_composition/runtime.rs")),
        (
            "play_continuation_bindings",
            include_str!("source_composition/play_continuation_bindings.rs"),
        ),
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
            "play_control",
            include_str!("source_composition/play_control.rs"),
        ),
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

#[cfg(test)]
mod tests {
    #[test]
    fn emitted_binding_count_is_the_same_tested_source() {
        let directory =
            std::env::temp_dir().join(format!("conduit-source-bindings-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        super::emit_to(&directory).unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.join("source_play_continuation_bindings.rs"))
                .unwrap(),
            include_str!("source_composition/play_continuation_bindings.rs")
        );
        let material =
            std::fs::read_to_string(directory.join("source_play_continuation.rs")).unwrap();
        assert!(material.contains("binding_count::distinct_fields(recipe)"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
