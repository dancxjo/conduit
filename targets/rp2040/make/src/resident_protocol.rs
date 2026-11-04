//! Target package installation closure for its selected HT16K33 implementation.
use conduit_embedded_build::{
    generate_resident_library, PreparedResidentExpression, ResidentExpressionBounds,
};
use conduit_host_make::CheckedHostConfiguration;
use conduit_plot as p;

/// Include the checked protocol implementation, without installing a Plot or bus
/// authority. Runtime must initialize the returned library before offering it.
pub fn checked_ht16k33_resident_library(
    checked: &CheckedHostConfiguration,
) -> Result<String, String> {
    crate::CheckedBoschHt16k33Construction::from_configuration(checked)?;
    let types = p::check_syntax_document(
        &p::parse_syntax_document(include_str!(
            "../../../../plots/device-protocols/i2c-types.conduit"
        )),
        &p::StartupCatalog::new(),
    )
    .map_err(|e| format!("I2C Source: {e:?}"))?;
    let mut catalog = p::StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        let identity = &types
            .native_types
            .iter()
            .find(|n| n.name == name)
            .ok_or("missing checked I2C Native owner")?
            .identity;
        catalog
            .insert_checked_native_type_from_document(path, &types, identity)
            .map_err(|e| format!("I2C Native import: {e:?}"))?;
    }
    let document = p::check_syntax_document(
        &p::parse_syntax_document(include_str!(
            "../../../../plots/device-protocols/ht16k33.conduit"
        )),
        &catalog,
    )
    .map_err(|e| format!("HT16K33 Source: {e:?}"))?;
    let mut images = Vec::new();
    for entry in [
        "ht16k33-initialize",
        "ht16k33-action",
        "ht16k33-transition",
        "ht16k33-frame-request",
        "ht16k33-write-outcome",
    ] {
        let expanded =
            p::expand_canonical_plot_for_authoring(&document, entry, &p::ProfileCatalog::new())
                .map_err(|e| format!("protocol expansion: {e:?}"))?;
        let [gear] = expanded.expanded.gears.as_slice() else {
            return Err("protocol entry is not one resident expression".into());
        };
        let [configuration] = gear.configuration.as_slice() else {
            return Err("protocol expression configuration absent".into());
        };
        let conduit_host_make::ConfigurationValue::Text(text) = &configuration.value else {
            return Err("protocol expression program absent".into());
        };
        if configuration.key != "program" {
            return Err("unsupported protocol computation".into());
        }
        let program = p::PortableExpressionProgram::from_canonical_hex(text)
            .map_err(|e| format!("protocol program: {e:?}"))?;
        // Exact package implementation ceilings, not runtime resource grants.
        let bounds = ResidentExpressionBounds {
            native: p::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: 1000,
        };
        let temporal = gear.inputs.first().ok_or("protocol input absent")?.temporal;
        images.push(
            PreparedResidentExpression::prepare(&document, &program, temporal, bounds)
                .map_err(|e| format!("resident protocol preparation: {e:?}"))?,
        );
    }
    generate_resident_library(&images, 2 * 1024 * 1024)
        .map_err(|e| format!("resident protocol library: {e:?}"))
}
