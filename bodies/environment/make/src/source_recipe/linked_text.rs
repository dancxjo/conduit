//! Emit exact immutable text as linked data, preserving the full public wire.
use conduit_core as c;
use std::fmt::{self, Write};

pub(crate) use conduit_host_make::linked_material::write_offer;

pub(crate) fn write_gear(
    out: &mut impl Write,
    name: &str,
    gear: &conduit_plot::CheckedGear,
) -> fmt::Result {
    let mut compact = gear.clone();
    for field in &mut compact.semantic_contract.configuration {
        field.default_value = c::ConfigurationValue::Bool(false);
    }
    for entry in &mut compact.configuration {
        entry.value = c::ConfigurationValue::Bool(false);
    }
    let literal = super::linked_rust::emit(&compact)?;
    let mutable =
        if gear.configuration.is_empty() && gear.semantic_contract.configuration.is_empty() {
            ""
        } else {
            "mut "
        };
    writeln!(out,"fn {name}() -> Result<conduit_plot::CheckedGear, conduit_plot::flat_image::ImageRefusal> {{ let {mutable}value: conduit_plot::CheckedGear = {literal};")?;
    for (index, field) in gear.semantic_contract.configuration.iter().enumerate() {
        writeln!(
            out,
            "value.semantic_contract.configuration[{index}].default_value = {};",
            super::linked_value::emit(&field.default_value)?
        )?;
    }
    for (index, entry) in gear.configuration.iter().enumerate() {
        writeln!(
            out,
            "value.configuration[{index}].value = {};",
            super::linked_value::emit(&entry.value)?
        )?;
    }
    writeln!(out, "Ok(value) }}")
}
/// Emit the exact configuration separately so fingerprinting does not decode
/// the selected Gear's unrelated contract arrays at its retained-Plan peak.
pub(crate) fn write_configuration(
    out: &mut impl Write,
    name: &str,
    gear: &conduit_plot::CheckedGear,
) -> fmt::Result {
    writeln!(out,"fn {name}() -> Result<conduit_plot::flat_image::indexed::IndexedGearConfiguration, conduit_plot::flat_image::ImageRefusal> {{ Ok(conduit_plot::flat_image::indexed::IndexedGearConfiguration {{ gear_id: {:?}.into(), configuration: alloc::vec![", gear.gear_id.as_str())?;
    for entry in &gear.configuration {
        writeln!(
            out,
            "c::ConfigurationEntry {{ key: {:?}.into(), value: {} }},",
            entry.key,
            super::linked_value::emit(&entry.value)?
        )?;
    }
    writeln!(out, "] }}) }}")
}
