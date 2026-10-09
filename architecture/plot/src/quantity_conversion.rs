//! Ordinary checked startup Gear for explicit exact conversion. The quoted
//! source retains authored spelling; no glyph or implicit rounding is needed.
//! Preparation produces a finite receipt, including a typed precision refusal.

mod encoding;
mod profile;
mod source;
use profile::ConversionProfile;
pub mod temperature_difference;
pub use source::{validate_source, QuantityConversionSourceDiagnostic};

use crate::{prelude::*, KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};
use conduit_core::*;

pub const KIND: &str = "units/convert";
pub const REVISION: &str = "quantity/exact-conversion@1";
pub const RECEIPT_NAME: &str = "ExactQuantityConversionReceipt";
pub const MAXIMUM_RECEIPT_BYTES: u32 = 8192;

fn leaf(identity: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(identity)).expect("reviewed primitive")
}
fn field(name: &str, ty: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, ty).expect("reviewed field")
}
fn coordinate_type(profile: ConversionProfile) -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(profile.coordinate_id()),
        vec![
            field("coefficient", leaf("value/i128")),
            field("exponent", leaf("value/i16")),
        ],
    )
    .expect("finite coordinate")
}
fn result_type(profile: ConversionProfile) -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id(profile.result_id()),
        vec![
            StructuredVariantCase::new("converted", coordinate_type(profile))
                .expect("reviewed case"),
            StructuredVariantCase::new("refused", leaf(TEXT_INFO_ID)).expect("reviewed case"),
        ],
    )
    .expect("finite result")
}
pub fn receipt_type() -> StructuredInfoType {
    receipt_type_for(ConversionProfile::Quantity)
}
fn receipt_type_for(profile: ConversionProfile) -> StructuredInfoType {
    let mut fields = vec![
        field("original", leaf(TEXT_INFO_ID)),
        field("source", profile.source_type()),
        field("source-suffix", leaf(TEXT_INFO_ID)),
        field("source-base", leaf(TEXT_INFO_ID)),
        field("source-prefix", leaf(TEXT_INFO_ID)),
        field("source-prefix-exponent", leaf("value/i16")),
        field("source-dimension", leaf(TEXT_INFO_ID)),
        field("target-dimension", leaf(TEXT_INFO_ID)),
        field("target", leaf(TEXT_INFO_ID)),
        field("profile", leaf(TEXT_INFO_ID)),
        field("catalogue", leaf(TEXT_INFO_ID)),
        field("target-exponent", leaf("value/i16")),
        field("result", result_type(profile)),
    ];
    for name in [
        "source-scale",
        "source-offset",
        "source-denominator",
        "target-scale",
        "target-offset",
        "target-denominator",
    ] {
        fields.push(field(name, leaf("value/i128")));
    }
    StructuredInfoType::record(kind_id(profile.receipt_id()), fields).expect("finite receipt")
}
pub fn install(startup: &mut StartupCatalog, profile: &mut ProfileCatalog) -> Result<(), String> {
    install_for(ConversionProfile::Quantity, startup, profile)?;
    install_for(ConversionProfile::TemperatureDifference, startup, profile)
}
fn install_for(
    selected: ConversionProfile,
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.ensure_structured_type(selected.receipt_name(), receipt_type_for(selected))?;
    startup.insert(KindSignature {
        kind: selected.kind().into(),
        startup_parameters: ["source", "to"]
            .into_iter()
            .map(|name| StartupParameterSignature {
                name: name.into(),
                value_type: "Text".into(),
                default: None,
            })
            .collect(),
    })?;
    profile
        .insert_kind(contract_for(selected))
        .map_err(|error| format!("{error}"))?;
    Ok(())
}
pub fn contract() -> Kind {
    contract_for(ConversionProfile::Quantity)
}
fn contract_for(profile: ConversionProfile) -> Kind {
    let output = receipt_type_for(profile)
        .profile()
        .expect("finite receipt")
        .value_kind()
        .clone();
    Kind {
        kind_id: kind_id(profile.kind()),
        kind_contract_revision: KindIdentity::from(profile.revision()),
        startup_parameters: ["source", "to"]
            .into_iter()
            .map(|name| FrontStartupParameter {
                name: name.into(),
                value_type: kind_id(TEXT_INFO_ID),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("receipt"),
            value_kind: output.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: ["source", "to"]
            .into_iter()
            .map(|name| KindConfigurationField {
                key: name.into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::TextBytes {
                    maximum: EXACT_DECIMAL_MAX_LITERAL_BYTES as u32,
                },
            })
            .collect(),
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Output(port_id("receipt")),
            contract: CheckedValueContract::new(output, MAXIMUM_RECEIPT_BYTES, vec![])
                .expect("finite receipt"),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_RECEIPT_BYTES,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuantityConversionPreparationRefusal {
    SourceCorrelation,
    ForgedReceipt,
    Configuration,
    Request(ExactQuantityConversionRequestRefusal),
    Receipt(StructuredInfoRefusal),
    ReceiptTooLarge,
}

/// Execute after ordinary startup checking and canonical expansion. Every
/// semantic value and refusal is retained in the receipt; no conversion is
/// retried with a different unit or silently projected to legacy storage.
pub fn prepare_configuration(
    configuration: &[ConfigurationEntry],
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    prepare_for(ConversionProfile::Quantity, configuration)
}
fn prepare_for(
    profile: ConversionProfile,
    configuration: &[ConfigurationEntry],
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    if configuration.len() != 2 {
        return Err(R::Configuration);
    }
    let parameter = |name: &str| -> Result<&str, R> {
        let mut entries = configuration.iter().filter(|entry| entry.key == name);
        let Some(entry) = entries.next() else {
            return Err(R::Configuration);
        };
        if entries.next().is_some() {
            return Err(R::Configuration);
        }
        match &entry.value {
            ConfigurationValue::Text(text) => Ok(text),
            _ => Err(R::Configuration),
        }
    };
    encoding::prepare(profile, parameter("source")?, parameter("to")?)
}

fn dimension_name(dimension: QuantityDimension) -> &'static str {
    match dimension {
        QuantityDimension::Time => "time",
        QuantityDimension::Frequency => "frequency",
        QuantityDimension::Voltage => "voltage",
        QuantityDimension::Current => "current",
        QuantityDimension::Temperature => "temperature",
        QuantityDimension::Charge => "charge",
        QuantityDimension::Length => "length",
        QuantityDimension::Angle => "angle",
        QuantityDimension::Ratio => "ratio",
        QuantityDimension::DataSize => "data-size",
        QuantityDimension::PixelCount => "pixel-count",
        QuantityDimension::Mass => "mass",
        QuantityDimension::Area => "area",
        QuantityDimension::Volume => "volume",
        QuantityDimension::Speed => "speed",
        QuantityDimension::Acceleration => "acceleration",
        QuantityDimension::Force => "force",
        QuantityDimension::Energy => "energy",
        QuantityDimension::Power => "power",
        QuantityDimension::Pressure => "pressure",
    }
}

/// Re-admission checks semantic facts, not merely the record's decoded shape.
/// A modified result, descriptor, profile or affine law is never a receipt.
/// This bounded recomputation belongs to preparation, not Play.
pub fn validate_receipt(
    receipt: &StructuredInfoValue,
) -> Result<(), QuantityConversionPreparationRefusal> {
    validate_for(ConversionProfile::Quantity, receipt)
}
fn validate_for(
    profile: ConversionProfile,
    receipt: &StructuredInfoValue,
) -> Result<(), QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    if receipt.value_type() != &receipt_type_for(profile) {
        return Err(R::ForgedReceipt);
    }
    if receipt.canonical_bytes().map_err(R::Receipt)?.len() > MAXIMUM_RECEIPT_BYTES as usize {
        return Err(R::ReceiptTooLarge);
    }
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        return Err(R::ForgedReceipt);
    };
    let parameter = |name: &str| -> Result<String, R> {
        let field = fields
            .iter()
            .find(|field| field.name() == name)
            .ok_or(R::ForgedReceipt)?;
        let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
            return Err(R::ForgedReceipt);
        };
        if bytes.len() > EXACT_DECIMAL_MAX_LITERAL_BYTES {
            return Err(R::ForgedReceipt);
        }
        core::str::from_utf8(bytes)
            .map(|text| text.into())
            .map_err(|_| R::ForgedReceipt)
    };
    let expected = prepare_for(
        profile,
        &[
            ConfigurationEntry {
                key: "source".into(),
                value: ConfigurationValue::Text(parameter("original")?),
            },
            ConfigurationEntry {
                key: "to".into(),
                value: ConfigurationValue::Text(parameter("target")?),
            },
        ],
    )?;
    if receipt != &expected {
        return Err(R::ForgedReceipt);
    }
    Ok(())
}
