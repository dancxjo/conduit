use alloc::vec::Vec;

use crate::{FormSyntax, RuntimePortDirection, RuntimePortTemporal, SyntaxCheckDiagnostic};
use conduit_core::{
    data_reference_kind, kind_id, CheckedFront, CheckedValueContract, FrontStartupParameter,
    FrontValueContract, FrontValueLocation, KindId, PortDescriptor, PortDirection,
    StructuredInfoRefusal,
};

use crate::StartupCatalog;

pub(crate) mod refinement;
use refinement::checked_refinements;

pub(crate) fn canonical_value_kind(source_type: &str) -> KindId {
    if let Some(value_type) = source_type.strip_prefix('&') {
        return data_reference_kind(&canonical_value_kind(value_type));
    }
    match source_type {
        "Text" => kind_id("value/text"),
        "Tick" => kind_id("value/tick@1"),
        "Count" => kind_id("value/count"),
        "Boolean" => kind_id("value/bool"),
        "Scalar" => kind_id("value/scalar"),
        "Bytes" => kind_id("value/bytes"),
        "Unit" => kind_id("value/unit"),
        "Quantity" => kind_id("value/quantity"),
        "QuantityUnit" => kind_id(conduit_core::QUANTITY_UNIT_INFO_ID),
        "U8" => kind_id("value/u8"),
        "U16" => kind_id("value/u16"),
        "U32" => kind_id("value/u32"),
        "U64" => kind_id("value/u64"),
        "U128" => kind_id("value/u128"),
        "I8" => kind_id("value/i8"),
        "I16" => kind_id("value/i16"),
        "I32" => kind_id("value/i32"),
        "I64" => kind_id("value/i64"),
        "I128" => kind_id("value/i128"),
        "F32" => kind_id(conduit_core::F32_INFO_ID),
        "F64" => kind_id(conduit_core::F64_INFO_ID),
        "Distance" => kind_id(conduit_core::DISTANCE_INFO_ID),
        "Frequency" => kind_id(conduit_core::FREQUENCY_INFO_ID),
        "Duration" => kind_id(conduit_core::DURATION_INFO_ID),
        "Voltage" => kind_id(conduit_core::VOLTAGE_INFO_ID),
        "Temperature" => kind_id(conduit_core::TEMPERATURE_INFO_ID),
        "Angle" => kind_id(conduit_core::ANGLE_INFO_ID),
        "Ratio" => kind_id(conduit_core::RATIO_INFO_ID),
        "PixelCount" => kind_id(conduit_core::PIXEL_COUNT_INFO_ID),
        "Pool" => kind_id("value/pool-reference"),
        exact => kind_id(exact),
    }
}

pub(crate) fn checked_value_kind(
    source_type: &str,
    catalog: &StartupCatalog,
) -> Result<KindId, StructuredInfoRefusal> {
    if let Some(value_type) = source_type.strip_prefix('&') {
        if value_type.is_empty() || value_type.starts_with('&') {
            return Err(StructuredInfoRefusal::WrongType);
        }
        return Ok(data_reference_kind(&checked_value_kind(
            value_type, catalog,
        )?));
    }
    if let Some(value_kind) = catalog.value_kind_alias(source_type) {
        return Ok(value_kind.clone());
    }
    catalog
        .structured_type(source_type)
        .map(|value_type| {
            value_type
                .profile()
                .map(|profile| profile.value_kind().clone())
        })
        .unwrap_or_else(|| {
            let canonical = canonical_value_kind(source_type);
            if canonical.as_str() == source_type && !source_type.contains('/') {
                Err(StructuredInfoRefusal::WrongType)
            } else {
                Ok(canonical)
            }
        })
}

pub(crate) fn checked_value_type(
    source_type: &str,
    catalog: &StartupCatalog,
) -> Result<conduit_core::StructuredInfoType, StructuredInfoRefusal> {
    if let Some(value_type) = catalog.structured_type(source_type) {
        return Ok(value_type.clone());
    }
    conduit_core::StructuredInfoType::leaf(checked_value_kind(source_type, catalog)?)
}

pub(crate) fn checked_optional_type(
    source_type: &str,
    catalog: &StartupCatalog,
) -> Result<conduit_core::StructuredInfoType, StructuredInfoRefusal> {
    conduit_core::optional_info_type(checked_value_type(source_type, catalog)?)
}

pub(crate) fn checked_front(
    form: &FormSyntax,
    catalog: &StartupCatalog,
) -> Result<CheckedFront, SyntaxCheckDiagnostic> {
    let startup_parameters = form
        .front
        .startup_parameters
        .iter()
        .map(|parameter| {
            Ok(FrontStartupParameter {
                name: parameter.name.text.clone(),
                value_type: checked_value_kind_with_modality(
                    &parameter.value_type.text,
                    parameter.optional,
                    catalog,
                )
                .map_err(|_| SyntaxCheckDiagnostic {
                    code: "CND-FRM-053",
                    span: parameter.value_type.span,
                    message: "structured startup parameter profile exceeds canonical bounds".into(),
                })?,
                // Presence is callable compatibility: callers need to know
                // whether omission is legal. The checked form identity owns
                // the canonical default expression and therefore its meaning.
                has_default: parameter.default.is_some(),
            })
        })
        .collect::<Result<Vec<_>, SyntaxCheckDiagnostic>>()?;
    let mut value_contracts = form
        .front
        .startup_parameters
        .iter()
        .filter(|parameter| parameter.maximum_bytes.is_some() || !parameter.refinements.is_empty())
        .map(|parameter| {
            let checked = startup_parameters
                .iter()
                .find(|checked| checked.name == parameter.name.text)
                .expect("checked startup parameter preserves its name");
            let (maximum_bytes, constraints) = checked_refinements(
                &parameter.refinements,
                parameter.maximum_bytes,
                &checked.value_type,
                parameter.value_type.span,
            )?;
            Ok(FrontValueContract {
                location: FrontValueLocation::Startup(parameter.name.text.clone()),
                contract: CheckedValueContract::new(
                    checked.value_type.clone(),
                    maximum_bytes,
                    constraints,
                )
                .map_err(|error| SyntaxCheckDiagnostic {
                    code: "CND-FRM-053",
                    span: parameter.value_type.span,
                    message: alloc::format!("startup value contract is invalid: {error:?}"),
                })?,
            })
        })
        .collect::<Result<Vec<_>, SyntaxCheckDiagnostic>>()?;
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for port in &form.front.runtime_ports {
        let descriptor = PortDescriptor {
            port_id: conduit_core::port_id(&port.name.text),
            value_kind: checked_value_kind_with_modality(
                &port.value_type.text,
                matches!(
                    port.temporal,
                    RuntimePortTemporal::OptionalValue | RuntimePortTemporal::CurrentOptional
                ),
                catalog,
            )
            .map_err(|_| SyntaxCheckDiagnostic {
                code: "CND-FRM-053",
                span: port.value_type.span,
                message: "structured runtime Port profile exceeds canonical bounds".into(),
            })?,
            direction: match port.direction {
                RuntimePortDirection::Input => PortDirection::Input,
                RuntimePortDirection::Output => PortDirection::Output,
            },
            temporal: canonical_port_temporal(port.temporal),
            abnormal_kind: None,
        };
        if port.maximum_bytes.is_some() || !port.refinements.is_empty() {
            let (maximum_bytes, constraints) = checked_refinements(
                &port.refinements,
                port.maximum_bytes,
                &descriptor.value_kind,
                port.value_type.span,
            )?;
            value_contracts.push(FrontValueContract {
                location: match port.direction {
                    RuntimePortDirection::Input => {
                        FrontValueLocation::Input(descriptor.port_id.clone())
                    }
                    RuntimePortDirection::Output => {
                        FrontValueLocation::Output(descriptor.port_id.clone())
                    }
                },
                contract: CheckedValueContract::new(
                    descriptor.value_kind.clone(),
                    maximum_bytes,
                    constraints,
                )
                .map_err(|error| SyntaxCheckDiagnostic {
                    code: "CND-FRM-053",
                    span: port.value_type.span,
                    message: alloc::format!("runtime Port value contract is invalid: {error:?}"),
                })?,
            });
        }
        match descriptor.direction {
            PortDirection::Input => inputs.push(descriptor),
            PortDirection::Output => outputs.push(descriptor),
        }
    }
    Ok(CheckedFront::new(
        startup_parameters,
        inputs,
        outputs,
        form.front.shorthand.as_ref().map(|pair| {
            (
                conduit_core::port_id(&pair.input_port.text),
                conduit_core::port_id(&pair.output_port.text),
            )
        }),
    )
    .with_value_contracts(value_contracts))
}

pub(crate) fn canonical_port_temporal(source: RuntimePortTemporal) -> conduit_core::PortTemporal {
    match source {
        RuntimePortTemporal::Value | RuntimePortTemporal::OptionalValue => {
            conduit_core::PortTemporal::Value
        }
        RuntimePortTemporal::Flow { closes } => conduit_core::PortTemporal::Flow { closes },
        RuntimePortTemporal::Current | RuntimePortTemporal::CurrentOptional => {
            conduit_core::PortTemporal::Current
        }
    }
}

pub(crate) fn checked_value_kind_with_modality(
    source_type: &str,
    optional: bool,
    catalog: &StartupCatalog,
) -> Result<KindId, StructuredInfoRefusal> {
    let value_kind = checked_value_kind(source_type, catalog)?;
    Ok(if optional {
        checked_optional_type(source_type, catalog)?
            .profile()?
            .value_kind()
            .clone()
    } else {
        value_kind
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_text_resolves_without_changing_exact_explicit_kinds() {
        assert_eq!(canonical_value_kind("Text").as_str(), "value/text");
        assert_eq!(canonical_value_kind("Tick").as_str(), "value/tick@1");
        assert_eq!(canonical_value_kind("Count").as_str(), "value/count");
        assert_eq!(
            canonical_value_kind("Distance").as_str(),
            conduit_core::DISTANCE_INFO_ID
        );
        assert_eq!(
            canonical_value_kind("Frequency").as_str(),
            conduit_core::FREQUENCY_INFO_ID
        );
        assert_eq!(
            canonical_value_kind("Duration").as_str(),
            conduit_core::DURATION_INFO_ID
        );
        assert_eq!(
            canonical_value_kind("Pool").as_str(),
            "value/pool-reference"
        );
        assert_eq!(canonical_value_kind("test/value").as_str(), "test/value");
        assert_eq!(canonical_value_kind("U32").as_str(), "value/u32");
        assert_eq!(
            canonical_value_kind("&Text").as_str(),
            "data/generation-reference<value/text>"
        );
    }

    #[test]
    fn semantic_owner_can_register_a_value_spelling_without_form_changes() {
        let mut catalog = StartupCatalog::new();
        catalog
            .insert_value_kind_alias("WeatherMap", kind_id("weather/map@1"))
            .unwrap();

        assert_eq!(
            checked_value_kind("WeatherMap", &catalog).unwrap().as_str(),
            "weather/map@1"
        );
        assert_eq!(
            checked_value_kind("weather/exact-map@2", &catalog)
                .unwrap()
                .as_str(),
            "weather/exact-map@2"
        );
    }

    #[test]
    fn unregistered_author_alias_refuses_before_planning() {
        assert_eq!(
            checked_value_kind("WeatherMap", &StartupCatalog::new()),
            Err(StructuredInfoRefusal::WrongType)
        );
        assert_eq!(
            checked_value_kind("weather/exact-map@2", &StartupCatalog::new())
                .unwrap()
                .as_str(),
            "weather/exact-map@2"
        );
    }

    #[test]
    fn data_reference_wraps_the_exact_checked_content_type_once() {
        let mut catalog = StartupCatalog::new();
        catalog
            .insert_value_kind_alias("Image", kind_id("media/image@4"))
            .unwrap();

        assert_eq!(
            checked_value_kind("&Image", &catalog).unwrap().as_str(),
            "data/generation-reference<media/image@4>"
        );
        assert_eq!(
            checked_value_kind("&&Image", &catalog),
            Err(StructuredInfoRefusal::WrongType)
        );
    }
}
