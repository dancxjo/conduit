//! Portable Form-facing Lenia contracts.

use alloc::{format, string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

pub const ORBIUM_SEED_KIND: &str = "alife/orbium-seed";
pub const LENIA_STEP_KIND: &str = "alife/lenia-step";
pub const SCALAR_FIELD_PRESENTATION_KIND: &str = "presentation/scalar-field";

pub const ORBIUM_SEED_REVISION: &str = "conduit.alife/orbium-seed@1";
pub const LENIA_STEP_REVISION: &str = "conduit.alife/lenia-step@1";
pub const SCALAR_FIELD_PRESENTATION_REVISION: &str = "conduit.presentation/scalar-field@1";

pub const ORBIUM_WIDTH_KEY: &str = "width";
pub const ORBIUM_HEIGHT_KEY: &str = "height";
pub const SEED_KEY: &str = "seed";
pub const KERNEL_RADIUS_KEY: &str = "kernel_radius";
pub const KERNEL_MU_KEY: &str = "kernel_mu";
pub const KERNEL_SIGMA_KEY: &str = "kernel_sigma";
pub const GROWTH_MU_KEY: &str = "growth_mu";
pub const GROWTH_SIGMA_KEY: &str = "growth_sigma";
pub const DT_KEY: &str = "dt";
pub const BOUNDARY_KEY: &str = "boundary";
pub const NUMERIC_PROFILE_KEY: &str = "numeric_profile";
pub const TITLE_KEY: &str = "title";
pub const MINIMUM_KEY: &str = "minimum";
pub const MAXIMUM_KEY: &str = "maximum";
pub const MAXIMUM_PRESENTED_FIELDS: u16 = 4;

pub fn install_lenia_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for definition in lenia_definitions() {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().to_string(),
            startup_parameters: definition
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: value_type(&field.default_value).to_string(),
                    default: Some(render_default(&field.default_value)),
                })
                .collect(),
        })?;
        profile
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn lenia_definitions() -> Vec<KindDefinition> {
    vec![
        orbium_seed_definition(),
        lenia_step_definition(),
        scalar_field_presentation_definition(),
    ]
}

pub fn orbium_seed_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(ORBIUM_SEED_KIND),
        kind_contract_revision: KindContractRevision::from(ORBIUM_SEED_REVISION),
        inputs: Vec::new(),
        outputs: vec![field_port(
            "field",
            PortDirection::Output,
            PortTemporal::Value,
        )],
        configuration: vec![
            u64_field(ORBIUM_WIDTH_KEY, 128, 32, 128),
            u64_field(ORBIUM_HEIGHT_KEY, 128, 32, 128),
            u64_field(SEED_KEY, 1, 0, u64::MAX),
        ],
    }
}

pub fn lenia_step_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(LENIA_STEP_KIND),
        kind_contract_revision: KindContractRevision::from(LENIA_STEP_REVISION),
        inputs: vec![
            field_port("initial", PortDirection::Input, PortTemporal::Value),
            PortDescriptor {
                port_id: port_id("tick"),
                value_kind: kind_id(conduit_time::TICK_VALUE_KIND),
                direction: PortDirection::Input,
                temporal: PortTemporal::Flow { closes: true },
            },
        ],
        outputs: vec![field_port(
            "field",
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        configuration: vec![
            u64_field(
                KERNEL_RADIUS_KEY,
                13,
                1,
                u64::from(crate::LENIA_MAXIMUM_KERNEL_RADIUS),
            ),
            i64_field(KERNEL_MU_KEY, 500_000, 0, 1_000_000),
            i64_field(KERNEL_SIGMA_KEY, 150_000, 1, 1_000_000),
            i64_field(GROWTH_MU_KEY, 150_000, 0, 1_000_000),
            i64_field(GROWTH_SIGMA_KEY, 15_000, 1, 1_000_000),
            i64_field(DT_KEY, 100_000, 1, 1_000_000),
            text_choice(BOUNDARY_KEY, "wrap", &["wrap"]),
            text_choice(NUMERIC_PROFILE_KEY, "fixed-q16.16", &["fixed-q16.16"]),
        ],
    }
}

pub fn scalar_field_presentation_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(SCALAR_FIELD_PRESENTATION_KIND),
        kind_contract_revision: KindContractRevision::from(SCALAR_FIELD_PRESENTATION_REVISION),
        inputs: vec![field_port(
            "field",
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: Vec::new(),
        configuration: vec![
            ConfigurationField {
                key: TITLE_KEY.to_string(),
                default_value: ConfigurationValue::Text("Scalar field".to_string()),
                validation: ConfigurationRule::TextBytes { maximum: 64 },
            },
            i64_field(MINIMUM_KEY, 0, 0, 1_000_000),
            i64_field(MAXIMUM_KEY, 1_000_000, 0, 1_000_000),
        ],
    }
}

fn field_port(name: &str, direction: PortDirection, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(crate::SCALAR_FIELD2_INFO_ID),
        direction,
        temporal,
    }
}

fn u64_field(key: &str, default: u64, minimum: u64, maximum: u64) -> ConfigurationField {
    ConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(default),
        validation: ConfigurationRule::U64Range { minimum, maximum },
    }
}

fn i64_field(key: &str, default: i64, minimum: i64, maximum: i64) -> ConfigurationField {
    ConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::I64(default),
        validation: ConfigurationRule::I64Range { minimum, maximum },
    }
}

fn text_choice(key: &str, default: &str, values: &[&str]) -> ConfigurationField {
    ConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text(default.to_string()),
        validation: ConfigurationRule::TextOneOf {
            values: values.iter().map(|value| (*value).to_string()).collect(),
        },
    }
}

fn value_type(value: &ConfigurationValue) -> &'static str {
    match value {
        ConfigurationValue::U64(_) => "Count",
        ConfigurationValue::I64(_) => "Scalar",
        ConfigurationValue::Text(_) => "Text",
        ConfigurationValue::Bool(_) => "Boolean",
        ConfigurationValue::Structured(_) => "Structured",
    }
}

fn render_default(value: &ConfigurationValue) -> alloc::string::String {
    match value {
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => render_scalar(*value),
        ConfigurationValue::Text(value) => format!("\"{value}\""),
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::Structured(_) => "structured".to_string(),
    }
}

fn render_scalar(value: i64) -> alloc::string::String {
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    let whole = magnitude / conduit_core::Scalar::SCALE as u64;
    let fraction = magnitude % conduit_core::Scalar::SCALE as u64;
    let sign = if negative { "-" } else { "" };
    if fraction == 0 {
        format!("{sign}{whole}.0")
    } else {
        let mut fraction = format!("{fraction:06}");
        while fraction.ends_with('0') {
            fraction.pop();
        }
        format!("{sign}{whole}.{fraction}")
    }
}
