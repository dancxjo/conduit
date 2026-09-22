use alloc::vec::Vec;

use crate::{FormSyntax, RuntimePortDirection, RuntimePortTemporal, SyntaxCheckDiagnostic};
use conduit_core::{
    kind_id, CheckedFront, FrontStartupParameter, KindId, PortDescriptor, PortDirection,
    StructuredInfoRefusal,
};

use crate::StartupCatalog;

pub(crate) fn canonical_value_kind(source_type: &str) -> KindId {
    match source_type {
        "Text" => kind_id("value/text"),
        "Tick" => kind_id("value/tick@1"),
        "Count" => kind_id("value/count"),
        "Boolean" => kind_id("value/bool"),
        "Scalar" => kind_id("value/scalar"),
        "Bytes" => kind_id("value/bytes"),
        "Unit" => kind_id("value/unit"),
        "Quantity" => kind_id("value/quantity"),
        "Duration" => kind_id(conduit_core::QUANTITY_INFO_ID),
        "Pool" => kind_id("value/pool-reference"),
        exact => kind_id(exact),
    }
}

pub(crate) fn checked_value_kind(
    source_type: &str,
    catalog: &StartupCatalog,
) -> Result<KindId, StructuredInfoRefusal> {
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
                value_type: checked_value_kind(&parameter.value_type.text, catalog).map_err(
                    |_| SyntaxCheckDiagnostic {
                        code: "CND-FRM-053",
                        span: parameter.value_type.span,
                        message: "structured startup parameter profile exceeds canonical bounds"
                            .into(),
                    },
                )?,
                // Presence is callable compatibility: callers need to know
                // whether omission is legal. The checked form identity owns
                // the canonical default expression and therefore its meaning.
                has_default: parameter.default.is_some(),
            })
        })
        .collect::<Result<Vec<_>, SyntaxCheckDiagnostic>>()?;
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for port in &form.front.runtime_ports {
        let descriptor = PortDescriptor {
            port_id: conduit_core::port_id(&port.name.text),
            value_kind: checked_value_kind(&port.value_type.text, catalog).map_err(|_| {
                SyntaxCheckDiagnostic {
                    code: "CND-FRM-053",
                    span: port.value_type.span,
                    message: "structured runtime Port profile exceeds canonical bounds".into(),
                }
            })?,
            direction: match port.direction {
                RuntimePortDirection::Input => PortDirection::Input,
                RuntimePortDirection::Output => PortDirection::Output,
            },
            temporal: canonical_port_temporal(port.temporal),
        };
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
    ))
}

pub(crate) fn canonical_port_temporal(source: RuntimePortTemporal) -> conduit_core::PortTemporal {
    match source {
        RuntimePortTemporal::Value => conduit_core::PortTemporal::Value,
        RuntimePortTemporal::Flow { closes } => conduit_core::PortTemporal::Flow { closes },
        RuntimePortTemporal::Current => conduit_core::PortTemporal::Current,
    }
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
            canonical_value_kind("Duration").as_str(),
            conduit_core::QUANTITY_INFO_ID
        );
        assert_eq!(
            canonical_value_kind("Pool").as_str(),
            "value/pool-reference"
        );
        assert_eq!(canonical_value_kind("test/value").as_str(), "test/value");
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
}
