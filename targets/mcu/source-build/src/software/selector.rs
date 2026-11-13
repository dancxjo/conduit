//! Exact checked selector realization, shared by software and slot fabrication.
use super::{c, p, SoftwareRole};
pub(crate) fn selected(gear: &p::CheckedGear) -> Result<Option<(c::Kind, SoftwareRole)>, String> {
    let mut entries = gear
        .configuration
        .iter()
        .filter(|entry| entry.key == "selector");
    let Some(entry) = entries.next() else {
        return Ok(None);
    };
    let c::ConfigurationValue::Text(text) = &entry.value else {
        return Err("selector encoding".into());
    };
    if entries.next().is_some() {
        return Err("selector duplicate".into());
    }
    let selector =
        c::StructuredSelector::from_canonical_hex(text).map_err(|e| format!("selector: {e:?}"))?;
    let kind = conduit_semantic_catalog::structured_selector_contract(
        &selector,
        c::PortTemporal::Flow { closes: true },
    );
    let input_bytes = conduit_plot::maximum_prepared_canonical_value_bytes(selector.input_type())
        .map_err(|e| format!("selector input: {e:?}"))?;
    let output_bytes = conduit_plot::maximum_prepared_canonical_value_bytes(selector.output_type())
        .map_err(|e| format!("selector output: {e:?}"))?;
    let canonical = selector
        .canonical_bytes()
        .map_err(|e| format!("selector material: {e:?}"))?;
    Ok(Some((
        kind,
        SoftwareRole::Selector {
            canonical,
            input_bytes: input_bytes as usize,
            output_bytes: output_bytes as usize,
        },
    )))
}
