//! Emit resident Back inventory from explicitly selected checked Source entries.
//! This accepts no raw Source, hardware selection, resource grant or Play authority.
use crate::{generate_resident_library, PreparedResidentExpression, ResidentExpressionBounds};
use conduit_plot as p;
pub fn generate_source_resident_library(
    document: &p::CheckedSyntaxDocument,
    entries: &[&str],
    bounds: ResidentExpressionBounds,
    maximum_generated_bytes: u32,
) -> Result<String, String> {
    let images = prepare_source_resident_library(document, entries, bounds)?;
    generate_resident_library(&images, maximum_generated_bytes)
        .map_err(|e| format!("resident protocol library: {e:?}"))
}
/// Prepare exact selected Source owners once for both structural calibration
/// and Rust inventory emission. This creates definitions, never Ready custody.
pub fn prepare_source_resident_library(
    document: &p::CheckedSyntaxDocument,
    entries: &[&str],
    bounds: ResidentExpressionBounds,
) -> Result<Vec<PreparedResidentExpression>, String> {
    let mut images = Vec::new();
    for entry in entries {
        let expanded =
            p::expand_canonical_plot_for_authoring(document, entry, &p::ProfileCatalog::new())
                .map_err(|e| format!("protocol expansion: {e:?}"))?;
        let [gear] = expanded.expanded.gears.as_slice() else {
            return Err("protocol entry is not one resident expression".into());
        };
        let [configuration] = gear.configuration.as_slice() else {
            return Err("protocol expression configuration absent".into());
        };
        let conduit_core::ConfigurationValue::Text(text) = &configuration.value else {
            return Err("protocol expression program absent".into());
        };
        if configuration.key != "program" {
            return Err("unsupported protocol computation".into());
        }
        let program = p::PortableExpressionProgram::from_canonical_hex(text)
            .map_err(|e| format!("protocol program: {e:?}"))?;
        let temporal = gear.inputs.first().ok_or("protocol input absent")?.temporal;
        images.push(
            PreparedResidentExpression::prepare(document, &program, temporal, bounds)
                .map_err(|e| format!("resident protocol preparation: {e:?}"))?,
        );
    }
    Ok(images)
}
