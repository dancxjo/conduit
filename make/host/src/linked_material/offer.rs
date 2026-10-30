//! Complete immutable offer construction text, never provider readiness.
use conduit_core as c;
use std::fmt::{self, Write};

pub fn write_offer(out: &mut impl Write, name: &str, offer: &c::CapabilityOffer) -> fmt::Result {
    let mut compact = offer.clone();
    for field in &mut compact.semantic_contract.configuration {
        field.default_value = c::ConfigurationValue::Bool(false);
    }
    writeln!(
        out,
        "fn {name}() -> Result<c::CapabilityOffer, ()> {{ let mut value = c::CapabilityOffer {{"
    )?;
    macro_rules! field {($($field:ident),*) => {$(writeln!(out,"{}: {},",stringify!($field),super::emit_rust_expression(&compact.$field)?)?;)*}}
    field!(
        startup_parameters,
        shorthand,
        capability_id,
        kind_id,
        kind_contract_revision,
        inputs,
        outputs,
        semantic_contract,
        implementation,
        state_retention,
        host_calls,
        resource_requirements,
        authority_requirements,
        limits
    );
    writeln!(out, "realization_properties: alloc::vec![")?;
    for property in &offer.realization_properties {
        writeln!(out,"match {} {{ c::ConfigurationValue::Structured(value) => value, _ => unreachable!() }},",super::emit_configuration_value(&c::ConfigurationValue::Structured(property.clone()))?)?;
    }
    writeln!(out, "] }};")?;
    for (index, field) in offer.semantic_contract.configuration.iter().enumerate() {
        writeln!(
            out,
            "value.semantic_contract.configuration[{index}].default_value = {};",
            super::emit_configuration_value(&field.default_value)?
        )?;
    }
    writeln!(out, "Ok(value) }}")
}
