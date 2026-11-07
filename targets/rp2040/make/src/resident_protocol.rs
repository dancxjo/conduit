//! Target package installation closure for its selected HT16K33 implementation.
use conduit_embedded_build::ResidentExpressionBounds;
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
    resident_entries(
        &document,
        &[
            "ht16k33-initialize",
            "ht16k33-action",
            "ht16k33-transition",
            "ht16k33-frame-request",
            "ht16k33-write-outcome",
        ],
    )
}
/// Exact Source decoder inventory. Electrical capture remains a separately
/// admitted provider; these expressions do not acquire the DHT pin.
pub fn checked_dht11_resident_images(
    checked: &CheckedHostConfiguration,
) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
    crate::CheckedDht11Tm1637Construction::from_configuration(checked)?;
    let document = conduit_mcu_source_build::checked_dht11_tm1637_source()?;
    conduit_embedded_build::prepare_source_resident_library(
        &document,
        conduit_mcu_source_build::DHT11_TM1637_ENTRIES,
        ResidentExpressionBounds {
            native: p::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: conduit_mcu_source_build::DHT11_TM1637_PRIVATE_WORK_UNITS,
        },
    )
}
pub fn checked_dht11_resident_library(
    checked: &CheckedHostConfiguration,
) -> Result<String, String> {
    let images = checked_dht11_resident_images(checked)?;
    conduit_embedded_build::generate_resident_library(&images, 2 * 1024 * 1024)
        .map_err(|e| format!("resident library: {e:?}"))
}

fn resident_entries(
    document: &p::CheckedSyntaxDocument,
    entries: &[&str],
) -> Result<String, String> {
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
    conduit_embedded_build::generate_source_resident_library(
        document,
        entries,
        bounds,
        2 * 1024 * 1024,
    )
}
