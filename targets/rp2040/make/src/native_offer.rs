//! Checked construction emits a definition, never an initialized provider.
use conduit_host_make::CheckedHostConfiguration;

/// Fabricate the selected native definition without synthesizing a Ready Base.
/// Root must independently match the image to its actual configured controller.
pub fn checked_i2c_offer_image(
    checked: &CheckedHostConfiguration,
    artifact_id: &str,
    maximum_image_bytes: usize,
) -> Result<Option<Vec<u8>>, String> {
    let bus = crate::CheckedI2cConstruction::from_configuration(checked)?;
    if bus.maximum_call_operations == 0 {
        return Ok(None);
    }
    if bus.call_address_range.is_none() {
        return Err("native I2C selection lacks a checked address interval".into());
    }
    let contract = conduit_i2c_transaction::contract::I2cContract::prepare()
        .map_err(|e| format!("checked I2C contract: {e:?}"))?;
    let offer = conduit_i2c_transaction::installation::capability_offer(
        &contract,
        &conduit_core::ArtifactId::from(artifact_id),
    )
    .map_err(|e| format!("I2C definition: {e:?}"))?;
    conduit_embedded_build::generate_capability_offer_image(&offer, maximum_image_bytes)
        .map(Some)
        .map_err(|e| format!("I2C definition image: {e:?}"))
}
