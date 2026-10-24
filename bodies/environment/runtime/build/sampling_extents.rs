//! Immutable canonical extent facts from the independently checked protocol Types.
use std::{fmt::Write, fs, path::Path};

pub fn emit(checked: &conduit_plot::CheckedSyntaxDocument, out: &Path) {
    let mut source = String::from("pub(super) const VALUE_EXTENTS: &[(&str, u32)] = &[\n");
    for ty in &checked.native_types {
        let kind = ty.value_type.profile().unwrap().value_kind().clone();
        let bytes = conduit_plot::maximum_prepared_canonical_value_bytes(&ty.value_type).unwrap();
        writeln!(source, "({:?}, {bytes}),", kind.as_str()).unwrap();
    }
    // Absence selectors preserve the canonical Empty envelope, unlike the bare
    // Empty trigger. The protocol catalog independently installs this alias.
    let absence =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::EMPTY_INFO_ID))
            .unwrap();
    writeln!(
        source,
        "({:?}, {}),\n];",
        absence.profile().unwrap().value_kind().as_str(),
        conduit_plot::maximum_prepared_canonical_value_bytes(&absence).unwrap()
    )
    .unwrap();
    fs::write(out.join("sampling-value-extents.rs"), source).unwrap();
    let reading = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "Dht11Result")
        .unwrap();
    let bytes = conduit_plot::maximum_prepared_canonical_value_bytes(&reading.value_type).unwrap();
    fs::write(
        out.join("sampling-reading-extent.rs"),
        format!("const _: () = assert!({bytes} <= CAPACITIES[0] && {bytes} <= CAPACITIES[1]);\n"),
    )
    .unwrap();
}
