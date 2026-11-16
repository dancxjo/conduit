//! Equipment-neutral checked workset inventory emission. Runtime role aliases
//! belong only to the explicit legacy DHT/Mask/TM adapter below.
use super::{Path, Workset};
use std::fmt::Write;

fn i2c_clock(entry: &super::Entry) -> bool {
    let kinds = entry
        .plot
        .expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .filter(|kind| crate::software::is_native_kind(kind))
        .collect::<Vec<_>>();
    kinds.len() == 2
        && kinds.contains(&"machine/i2c/transact")
        && kinds.contains(&"machine/clock/at")
}

pub(super) fn emit(workset: &Workset, out: &Path, legacy_roles: bool) -> Result<(), String> {
    if !workset.calibrated {
        return Err(
            "runtime factory emission requires independent checked structural calibration".into(),
        );
    }
    let index = index(workset, legacy_roles)?;
    for (i, entry) in workset.entries.iter().enumerate() {
        std::fs::write(out.join(format!("workset_{i}.rs")), &entry.rust)
            .map_err(|e| e.to_string())?;
    }
    if workset.entries.iter().any(i2c_clock) {
        std::fs::write(
            out.join("source_native_i2c_clock.rs"),
            include_str!("source_composition/native_i2c_clock.rs"),
        )
        .map_err(|e| e.to_string())?;
    }
    std::fs::write(out.join("workset.rs"), index).map_err(|e| e.to_string())
}
fn index(workset: &Workset, legacy_roles: bool) -> Result<String, String> {
    let mut index=format!("pub const SOURCE_SHA256:[u8;32]={:?};\npub const ENTRY_COUNT:usize={};\npub const ENTRY_NAMES:&[&str]=&{:?};\n",workset.sha256,workset.entries.len(),workset.entries.iter().map(|e|e.name.as_str()).collect::<Vec<_>>());
    index.push_str(&workset.installed_values.emit_shared());
    if legacy_roles {
        // Bind this finite execution profile to actual checked operations,
        // independent of user entry names or the signed workset's ordering.
        for (role, kind) in [
            ("sampling", "machine/pulse/read"),
            ("mask", "face/renderer"),
            ("output", "machine/tm1637/write"),
        ] {
            let selected = workset
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry
                        .plot
                        .expanded
                        .gears
                        .iter()
                        .any(|g| g.kind_id.as_str() == kind)
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let [entry] = selected.as_slice() else {
                return Err(format!(
                    "execution profile requires exactly one checked {role} fragment"
                ));
            };
            writeln!(index, "pub use entry_{entry} as {role};").unwrap();
            writeln!(index, "pub use entry_{entry}_layout as {role}_layout;").unwrap();
        }
    }
    for (i, entry) in workset.entries.iter().enumerate() {
        let filename = format!("workset_{i}.rs");
        writeln!(
            index,
            "pub mod entry_{i}{{include!(concat!(env!(\"OUT_DIR\"),\"/{filename}\"));}}"
        )
        .unwrap();
        writeln!(index,"pub mod entry_{i}_layout{{include!(concat!(env!(\"OUT_DIR\"),\"/workset_{i}_layout.rs\"));}}\n").unwrap();
        // The executable constructor uses NATIVE_KINDS from the generated
        // software module: calibration may permute Source nodes into kernel
        // slots, so original expanded-gear indices are not executable nodes.
        if i2c_clock(entry) {
            writeln!(index, "pub mod entry_{i}_native{{\nuse super::entry_{i}_layout as layout;\nuse super::entry_{i} as source;\ninclude!(concat!(env!(\"OUT_DIR\"),\"/source_native_i2c_clock.rs\"));\n}}").unwrap();
        }
    }
    Ok(index)
}

#[cfg(test)]
use crate as source_build;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/fixtures/environmental_exchange_source.rs"]
mod environmental_source;

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn workset() -> Workset {
        let source = b"plot value (\n input: U64...| >> output: U64...|\n) = (.)\n";
        super::super::compile(
            source,
            &format!("{:x}", Sha256::digest(source)),
            &["value".into()],
            None,
        )
        .unwrap()
    }
    #[test]
    fn real_shared_bme_ht_source_selects_native_constructor() {
        let fixture = environmental_source::selected_source();
        let workset = fixture.compile().unwrap();
        let text = index(&workset, false).unwrap();
        assert!(text.contains("pub mod entry_0_native"));
        let kinds = &workset.entries[0].plot.expanded.gears;
        assert_eq!(
            kinds
                .iter()
                .filter(|g| g.kind_id.as_str() == "machine/i2c/transact")
                .count(),
            1
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|g| g.kind_id.as_str() == "machine/clock/at")
                .count(),
            1
        );
        assert!(!text.contains(" as sampling"));
    }
    #[test]
    fn neutral_index_preserves_source_entry_and_layout_without_dht_roles() {
        let workset = workset();
        let text = index(&workset, false).unwrap();
        assert!(text.contains(&format!("SOURCE_SHA256:[u8;32]={:?}", workset.sha256)));
        assert!(text.contains("ENTRY_NAMES:&[&str]=&[\"value\"]"));
        assert!(text.contains("pub mod entry_0{"));
        assert!(text.contains("pub mod entry_0_layout{"));
        assert!(!text.contains("entry_0_native"));
        assert!(!text.contains(" as sampling"));
        assert!(!text.contains(" as mask"));
        assert!(!text.contains(" as output"));
        assert!(index(&workset, true).unwrap_err().contains("sampling"));
    }
    #[test]
    fn neutral_emission_cannot_bypass_original_structural_calibration() {
        let workset = workset();
        let error = workset
            .emit_neutral_to(Path::new("unused-uncalibrated-output"))
            .unwrap_err();
        assert!(error.contains("independent checked structural calibration"));
    }
}
