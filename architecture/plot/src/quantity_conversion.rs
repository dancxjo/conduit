//! Ordinary checked startup Gear for explicit exact conversion. The quoted
//! source retains authored spelling; no glyph or implicit rounding is needed.
//! Preparation produces a finite receipt, including a typed precision refusal.

mod source;
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
fn coordinate_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("quantity/exact-target-coordinate@1"),
        vec![
            field("coefficient", leaf("value/i128")),
            field("exponent", leaf("value/i16")),
        ],
    )
    .expect("finite coordinate")
}
fn result_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("quantity/exact-conversion-result@1"),
        vec![
            StructuredVariantCase::new("converted", coordinate_type()).expect("reviewed case"),
            StructuredVariantCase::new("refused", leaf(TEXT_INFO_ID)).expect("reviewed case"),
        ],
    )
    .expect("finite result")
}
pub fn receipt_type() -> StructuredInfoType {
    let mut fields = vec![
        field("original", leaf(TEXT_INFO_ID)),
        field("source", leaf(EXACT_DECIMAL_QUANTITY_INFO_ID)),
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
        field("result", result_type()),
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
    StructuredInfoType::record(kind_id("quantity/exact-conversion-receipt@1"), fields)
        .expect("finite receipt")
}
pub fn install(startup: &mut StartupCatalog, profile: &mut ProfileCatalog) -> Result<(), String> {
    startup.ensure_structured_type(RECEIPT_NAME, receipt_type())?;
    startup.insert(KindSignature {
        kind: KIND.into(),
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
        .insert_kind(contract())
        .map_err(|error| format!("{error}"))?;
    Ok(())
}
pub fn contract() -> Kind {
    let output = receipt_type()
        .profile()
        .expect("finite receipt")
        .value_kind()
        .clone();
    Kind {
        kind_id: kind_id(KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
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
    let receipt = ExactQuantityConversionReceipt::check(parameter("source")?, parameter("to")?)
        .map_err(R::Request)?;
    let (ss, so, sd) = receipt.source_transform();
    let (ts, to, td) = receipt.target_transform();
    let value = |identity: &str, bytes: Vec<u8>| {
        StructuredInfoValue::leaf(leaf(identity), bytes).map_err(R::Receipt)
    };
    let text = |text: &str| value(TEXT_INFO_ID, text.as_bytes().to_vec());
    let result = match receipt.result() {
        Ok(coordinate) => {
            let coordinate = StructuredInfoValue::record(
                coordinate_type(),
                vec![
                    StructuredFieldValue::new(
                        "coefficient",
                        value(
                            "value/i128",
                            coordinate.coefficient().to_le_bytes().to_vec(),
                        )?,
                    )
                    .map_err(R::Receipt)?,
                    StructuredFieldValue::new(
                        "exponent",
                        value("value/i16", coordinate.exponent().to_le_bytes().to_vec())?,
                    )
                    .map_err(R::Receipt)?,
                ],
            )
            .map_err(R::Receipt)?;
            StructuredInfoValue::variant(result_type(), "converted", coordinate)
                .map_err(R::Receipt)?
        }
        Err(refusal) => StructuredInfoValue::variant(
            result_type(),
            "refused",
            text(match refusal {
                QuantityConversionRefusal::IncompatibleDimensions => "incompatible-dimensions",
                QuantityConversionRefusal::Inexact => "inexact",
                QuantityConversionRefusal::Overflow => "overflow",
            })?,
        )
        .map_err(R::Receipt)?,
    };
    let mut fields = vec![
        ("original", text(receipt.original())?),
        (
            "source",
            value(
                EXACT_DECIMAL_QUANTITY_INFO_ID,
                receipt.source().encode().to_vec(),
            )?,
        ),
        ("source-suffix", text(receipt.source_suffix().source())?),
        (
            "source-base",
            text(
                receipt
                    .source_suffix()
                    .base()
                    .map(|base| base.unit())
                    .unwrap_or(receipt.source().unit())
                    .plot_suffix(),
            )?,
        ),
        (
            "source-prefix",
            text(
                receipt
                    .source_suffix()
                    .prefix()
                    .map_or("", |prefix| prefix.symbol()),
            )?,
        ),
        (
            "source-prefix-exponent",
            value(
                "value/i16",
                i16::from(
                    receipt
                        .source_suffix()
                        .prefix()
                        .map_or(0, |prefix| prefix.exponent()),
                )
                .to_le_bytes()
                .to_vec(),
            )?,
        ),
        (
            "source-dimension",
            text(dimension_name(receipt.source().dimension()))?,
        ),
        (
            "target-dimension",
            text(dimension_name(
                receipt
                    .target()
                    .base()
                    .map(|base| base.unit())
                    .unwrap_or_else(|| receipt.target().legacy_unit().unwrap())
                    .dimension(),
            ))?,
        ),
        ("target", text(receipt.target().source())?),
        ("profile", text(EXACT_DECIMAL_QUANTITY_INFO_ID)?),
        ("catalogue", text(QUANTITY_PREFIX_CATALOG_ID)?),
        (
            "target-exponent",
            value(
                "value/i16",
                receipt
                    .target()
                    .decimal_exponent()
                    .unwrap_or(0)
                    .to_le_bytes()
                    .to_vec(),
            )?,
        ),
        ("result", result),
    ];
    for (name, number) in [
        ("source-scale", ss),
        ("source-offset", so),
        ("source-denominator", sd),
        ("target-scale", ts),
        ("target-offset", to),
        ("target-denominator", td),
    ] {
        fields.push((name, value("value/i128", number.to_le_bytes().to_vec())?));
    }
    let receipt = StructuredInfoValue::record(
        receipt_type(),
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()
            .map_err(R::Receipt)?,
    )
    .map_err(R::Receipt)?;
    if receipt.canonical_bytes().map_err(R::Receipt)?.len() > MAXIMUM_RECEIPT_BYTES as usize {
        return Err(R::ReceiptTooLarge);
    }
    Ok(receipt)
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
    use QuantityConversionPreparationRefusal as R;
    if receipt.value_type() != &receipt_type() {
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
    let expected = prepare_configuration(&[
        ConfigurationEntry {
            key: "source".into(),
            value: ConfigurationValue::Text(parameter("original")?),
        },
        ConfigurationEntry {
            key: "to".into(),
            value: ConfigurationValue::Text(parameter("target")?),
        },
    ])?;
    if receipt != &expected {
        return Err(R::ForgedReceipt);
    }
    Ok(())
}
