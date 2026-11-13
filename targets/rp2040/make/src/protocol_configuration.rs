//! Checked Host Base facts for the existing authored protocol input Types.
//! No physical address is embedded in portable Plot Source.
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_host_make::CheckedHostConfiguration;
use conduit_mcu_source_build::linked_source::LinkedSourceImage;
use conduit_plot::{CheckedNativeType, CheckedSyntaxDocument};

pub struct CheckedProtocolConfigurationImages {
    pub configuration_id: String,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub sensor: LinkedSourceImage,
    pub display: LinkedSourceImage,
}
impl CheckedProtocolConfigurationImages {
    /// Derive physical settings only from the exact selected Base implementations.
    /// The checked Source retains meaning/laws; the future Plan still selects
    /// each initialized producer, its original identity, resources and Start.
    pub fn prepare(
        checked: &CheckedHostConfiguration,
        source: &CheckedSyntaxDocument,
        blink: u8,
        enabled: bool,
    ) -> Result<Self, String> {
        if blink > 3 {
            return Err("unsupported authored display blink policy".into());
        }
        let hardware = crate::CheckedBoschHt16k33Construction::from_configuration(checked)?;
        let native = |name: &str| {
            let mut found = source.native_types.iter().filter(|ty| ty.name == name);
            let ty = found
                .next()
                .ok_or_else(|| format!("missing protocol Native Type {name}"))?;
            if found.next().is_some() {
                return Err(format!("ambiguous protocol Native Type {name}"));
            }
            Ok(ty)
        };
        let sensor = record(
            native("BmeProtocolBegin")?,
            &[("address", hardware.sensor_address)],
        )?;
        // Blink/enabled are explicit authored application policy, separate from
        // the selected Base address/brightness; this adapter chooses neither.
        let display = record(
            native("Ht16k33Configuration")?,
            &[
                ("address", hardware.display_address),
                ("brightness", hardware.brightness),
                ("blink", blink),
                ("enabled", u8::from(enabled)),
            ],
        )?;
        Ok(Self {
            configuration_id: hardware.bus.configuration_id,
            source_document_id: source.source_document_id.clone(),
            sensor,
            display,
        })
    }
}
fn record(native: &CheckedNativeType, values: &[(&str, u8)]) -> Result<LinkedSourceImage, String> {
    let StructuredInfoTypeShape::Record { fields, .. } = native.value_type.shape() else {
        return Err("protocol configuration must retain its checked record Type".into());
    };
    if fields.len() != values.len() {
        return Err("protocol configuration field shape changed".into());
    }
    let mut prepared = Vec::new();
    for field in fields {
        let value = values
            .iter()
            .find(|(name, _)| *name == field.name())
            .ok_or("protocol configuration field changed")?
            .1;
        let leaf = StructuredInfoValue::leaf(field.value_type().clone(), vec![value])
            .map_err(|e| format!("protocol configuration leaf: {e:?}"))?;
        prepared.push(
            StructuredFieldValue::new(field.name(), leaf)
                .map_err(|e| format!("protocol configuration field: {e:?}"))?,
        );
    }
    let value = StructuredInfoValue::record(native.value_type.clone(), prepared)
        .map_err(|e| format!("protocol configuration record: {e:?}"))?;
    LinkedSourceImage::prepare(native, &value)
}

#[cfg(test)]
#[path = "protocol_configuration_tests.rs"]
mod tests;
