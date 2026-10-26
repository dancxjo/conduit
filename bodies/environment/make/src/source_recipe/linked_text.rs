//! Emit exact immutable text as linked data, preserving the full public wire.
use conduit_core as c;
use std::fmt::{self, Write};

pub(crate) fn write_offer(
    out: &mut impl Write,
    name: &str,
    offer: &c::CapabilityOffer,
) -> fmt::Result {
    let mut compact = offer.clone();
    let texts = extract_defaults(&mut compact.semantic_contract);
    let json = serde_json::to_vec(&compact).map_err(|_| fmt::Error)?;
    writeln!(out, "fn {name}() -> Result<c::CapabilityOffer, ()> {{ let mut value: c::CapabilityOffer = serde_json::from_slice(&{json:?}).map_err(|_| ())?;")?;
    write_defaults(out, &texts)?;
    write_compaction(
        out,
        &[
            "startup_parameters",
            "inputs",
            "outputs",
            "semantic_contract.configuration",
            "semantic_contract.laws",
            "host_calls",
            "resource_requirements",
            "authority_requirements",
            "realization_properties",
        ],
    )?;
    writeln!(out, "Ok(value) }}")
}
pub(crate) fn write_gear(
    out: &mut impl Write,
    name: &str,
    gear: &conduit_plot::CheckedGear,
) -> fmt::Result {
    let mut compact = gear.clone();
    let defaults = extract_defaults(&mut compact.semantic_contract);
    let mut values = Vec::new();
    for (index, entry) in compact.configuration.iter_mut().enumerate() {
        if let c::ConfigurationValue::Text(text) = &mut entry.value {
            values.push((index, std::mem::take(text)));
        }
    }
    let json = serde_json::to_vec(&compact).map_err(|_| fmt::Error)?;
    writeln!(out, "fn {name}() -> Result<conduit_plot::CheckedGear, conduit_plot::flat_image::ImageRefusal> {{ let mut value: conduit_plot::CheckedGear = serde_json::from_slice(&{json:?}).map_err(|_| conduit_plot::flat_image::ImageRefusal::Representation)?;")?;
    write_defaults(out, &defaults)?;
    for (index, text) in values {
        writeln!(out, "value.configuration[{index}].value = c::ConfigurationValue::Text(c::ConfigurationText::from_static({:?}));", text.as_str())?;
    }
    write_compaction(
        out,
        &[
            "startup_parameters",
            "inputs",
            "outputs",
            "semantic_contract.configuration",
            "semantic_contract.laws",
            "configuration",
            "resource_ports",
            "terminal_transductions",
        ],
    )?;
    writeln!(out, "Ok(value) }}")
}
fn extract_defaults(contract: &mut c::KindSemanticContract) -> Vec<(usize, c::ConfigurationText)> {
    let mut texts = Vec::new();
    for (index, field) in contract.configuration.iter_mut().enumerate() {
        if let c::ConfigurationValue::Text(text) = &mut field.default_value {
            texts.push((index, std::mem::take(text)));
        }
    }
    texts
}
fn write_defaults(out: &mut impl Write, texts: &[(usize, c::ConfigurationText)]) -> fmt::Result {
    for (index, text) in texts {
        writeln!(out, "value.semantic_contract.configuration[{index}].default_value = c::ConfigurationValue::Text(c::ConfigurationText::from_static({:?}));", text.as_str())?;
    }
    Ok(())
}

fn write_compaction(out: &mut impl Write, fields: &[&str]) -> fmt::Result {
    // Serde's geometrically grown vectors are decoding scratch. Retain exactly
    // the reconstructed contract, including all laws and finite value bounds.
    for field in fields {
        writeln!(out, "value.{field}.shrink_to_fit();")?;
    }
    writeln!(out, "for law in &mut value.semantic_contract.laws {{ if let c::KindSemanticLaw::ValueContracts(contracts) = law {{ contracts.shrink_to_fit(); for contract in contracts {{ contract.contract.constraints.shrink_to_fit(); }} }} }}")
}

/// Emit the exact configuration separately so fingerprinting does not decode
/// the selected Gear's unrelated contract arrays at its retained-Plan peak.
pub(crate) fn write_configuration(
    out: &mut impl Write,
    name: &str,
    gear: &conduit_plot::CheckedGear,
) -> fmt::Result {
    let mut entries = gear.configuration.clone();
    let mut texts = Vec::new();
    for (index, entry) in entries.iter_mut().enumerate() {
        if let c::ConfigurationValue::Text(text) = &mut entry.value {
            texts.push((index, std::mem::take(text)));
        }
    }
    let json = serde_json::to_vec(&entries).map_err(|_| fmt::Error)?;
    writeln!(out, "fn {name}() -> Result<conduit_plot::flat_image::indexed::IndexedGearConfiguration, conduit_plot::flat_image::ImageRefusal> {{ let mut value: alloc::vec::Vec<c::ConfigurationEntry> = serde_json::from_slice(&{json:?}).map_err(|_| conduit_plot::flat_image::ImageRefusal::Representation)?;")?;
    for (index, text) in texts {
        writeln!(out, "value[{index}].value = c::ConfigurationValue::Text(c::ConfigurationText::from_static({:?}));", text.as_str())?;
    }
    writeln!(out, "value.shrink_to_fit(); Ok(conduit_plot::flat_image::indexed::IndexedGearConfiguration {{ gear_id: {:?}.into(), configuration: value }}) }}", gear.gear_id.as_str())
}
