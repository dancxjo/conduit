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
    // The two complete quantity-bearing observation/result Types use the large
    // class. Every other checked protocol Type must fit every ordinary slot.
    // This is a complete Type ceiling, not an observed-content bound or liveness
    // proof. A changed graph still requires renewing its nine-slot proof.
    let ordinary = checked
        .native_types
        .iter()
        .filter(|ty| !matches!(ty.name.as_str(), "Dht11Observation" | "Dht11Result"))
        .map(|ty| conduit_plot::maximum_prepared_canonical_value_bytes(&ty.value_type).unwrap())
        .max()
        .unwrap();
    let large = checked
        .native_types
        .iter()
        .map(|ty| conduit_plot::maximum_prepared_canonical_value_bytes(&ty.value_type).unwrap())
        .max()
        .unwrap();
    assert_eq!(
        large, bytes,
        "reading must cover all checked protocol Types"
    );
    fs::write(
        out.join("sampling-reading-extent.rs"),
        format!("const _: () = {{ assert!({large} <= CAPACITIES[0] && {large} <= CAPACITIES[1]); let mut i = 2; while i < CAPACITIES.len() {{ assert!({ordinary} <= CAPACITIES[i]); i += 1; }} }};\n"),
    ).unwrap();
}
