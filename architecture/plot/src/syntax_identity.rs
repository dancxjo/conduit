use crate::prelude::*;
use crate::{
    hash_string, CanonicalStartupValue, CheckedCanonicalCord, CheckedCanonicalGear,
    CheckedCordStage, CheckedStartupParameter, PlotCompletionPolicy,
};
use conduit_core::CheckedPlotId;

pub(crate) struct CheckedIdentityFront<'a> {
    pub parameters: &'a [CheckedStartupParameter],
    pub runtime_ports: &'a [crate::RuntimePort],
    pub runtime_front: &'a conduit_core::CheckedFront,
    pub shorthand: Option<(&'a str, &'a str)>,
}

pub(crate) fn checked_identity(
    meaning: (&str, PlotCompletionPolicy),
    front: CheckedIdentityFront<'_>,
    gears: &[CheckedCanonicalGear],
    cords: &[CheckedCanonicalCord],
    pools: &[crate::CheckedPoolDeclaration],
) -> CheckedPlotId {
    let (name, completion) = meaning;
    let CheckedIdentityFront {
        parameters,
        runtime_ports,
        runtime_front,
        shorthand,
    } = front;
    let mut canonical = String::from("canonical-plot");
    push_field(&mut canonical, name);
    push_field(
        &mut canonical,
        match completion {
            PlotCompletionPolicy::Live => "live",
            PlotCompletionPolicy::SemanticCompletion => "complete",
        },
    );
    for parameter in parameters {
        canonical.push_str("param");
        push_field(&mut canonical, &parameter.name);
        push_field(&mut canonical, &parameter.value_type);
        push_field(
            &mut canonical,
            if parameter.optional {
                "optional"
            } else {
                "required-value"
            },
        );
        push_field(
            &mut canonical,
            &parameter
                .maximum_bytes
                .map_or_else(|| "intrinsic".into(), |value| value.to_string()),
        );
        let default = parameter
            .default
            .as_ref()
            .map(canonical_value)
            .unwrap_or_else(|| "required".into());
        push_field(&mut canonical, &default);
    }
    for port in runtime_front.inputs().iter().chain(runtime_front.outputs()) {
        canonical.push_str("port");
        push_field(&mut canonical, port.port_id.as_str());
        push_field(&mut canonical, port.value_kind.as_str());
        push_field(&mut canonical, &format!("{:?}", port.direction));
        push_field(&mut canonical, port.temporal.as_str());
    }
    for port in runtime_ports {
        canonical.push_str("port-contract");
        push_field(&mut canonical, &port.name.text);
        push_field(&mut canonical, &format!("{:?}", port.temporal));
        push_field(
            &mut canonical,
            &port
                .maximum_bytes
                .map_or_else(|| "intrinsic".into(), |value| value.to_string()),
        );
    }
    for contract in runtime_front
        .value_contracts()
        .iter()
        .filter(|contract| !contract.contract.constraints.is_empty())
    {
        canonical.push_str("value-contract");
        push_field(&mut canonical, &format!("{:?}", contract.location));
        let mut encoded = String::new();
        push_hex(&mut encoded, &contract.contract.identity_bytes());
        push_field(&mut canonical, &encoded);
    }
    if let Some((input, output)) = shorthand {
        canonical.push_str("shorthand");
        push_field(&mut canonical, input);
        push_field(&mut canonical, output);
    }
    for gear in gears {
        canonical.push_str("gear");
        push_field(&mut canonical, &canonical_gear(gear));
    }
    for cord in cords {
        canonical.push_str("cord");
        push_field(&mut canonical, &canonical_cord(cord));
    }
    for pool in pools {
        canonical.push_str("pool");
        push_field(&mut canonical, &pool.name);
        push_field(&mut canonical, &pool.maximum_members.to_string());
        for parameter in pool.member_front.startup_parameters() {
            push_field(&mut canonical, &parameter.name);
            push_field(&mut canonical, parameter.value_type.as_str());
            push_field(
                &mut canonical,
                if parameter.has_default {
                    "default"
                } else {
                    "required"
                },
            );
        }
        for port in pool
            .member_front
            .inputs()
            .iter()
            .chain(pool.member_front.outputs())
        {
            push_field(&mut canonical, port.port_id.as_str());
            push_field(&mut canonical, port.value_kind.as_str());
            push_field(&mut canonical, port.temporal.as_str());
            push_field(&mut canonical, &format!("{:?}", port.direction));
        }
        if let Some((input, output)) = pool.member_front.shorthand() {
            push_field(&mut canonical, input.as_str());
            push_field(&mut canonical, output.as_str());
        }
    }
    CheckedPlotId::from(hash_string(&canonical))
}

pub(crate) fn canonical_gear(gear: &CheckedCanonicalGear) -> String {
    let mut value = String::new();
    push_field(&mut value, gear.name.as_deref().unwrap_or("<anonymous>"));
    push_field(&mut value, &gear.kind);
    for binding in &gear.startup_bindings {
        push_field(&mut value, &binding.name);
        push_field(&mut value, &binding.value_type);
        push_field(&mut value, &canonical_value(&binding.value));
    }
    if let Some(retained) = &gear.retained {
        push_field(&mut value, "keep");
        push_field(&mut value, retained.value_kind.as_str());
        push_field(
            &mut value,
            if retained.optional {
                "optional"
            } else {
                "required-value"
            },
        );
        push_field(
            &mut value,
            &retained
                .maximum_bytes
                .map_or_else(|| "intrinsic".into(), |bound| bound.to_string()),
        );
        push_field(&mut value, &format!("{:?}", retained.duration));
        push_field(
            &mut value,
            &retained
                .initial
                .as_ref()
                .map_or_else(|| "none".into(), canonical_value),
        );
    }
    if let Some(activation) = &gear.activation {
        push_field(
            &mut value,
            match activation.mode {
                crate::ActivationSyntax::Each { .. } => "activate-each",
                crate::ActivationSyntax::Select { .. } => "activate-select",
                crate::ActivationSyntax::Fold { .. } => "activate-fold",
                crate::ActivationSyntax::Scan { .. } => "activate-scan",
            },
        );
        push_field(&mut value, &activation.mode.maximum_items().to_string());
        push_field(&mut value, &activation.selected_plot);
        push_field(&mut value, activation.input.port_id.as_str());
        push_field(&mut value, activation.input.value_kind.as_str());
        push_field(&mut value, activation.output.port_id.as_str());
        push_field(&mut value, activation.output.value_kind.as_str());
        push_field(
            &mut value,
            activation
                .input
                .abnormal_kind
                .as_ref()
                .map_or("normal", conduit_core::KindId::as_str),
        );
        if let Some(accumulator) = &activation.accumulator_input {
            push_field(&mut value, accumulator.port_id.as_str());
            push_field(&mut value, accumulator.value_kind.as_str());
        }
        if let Some(initial) = &activation.initial_accumulator {
            push_field(&mut value, &canonical_value(initial));
        }
        if let Some(bytes) = &activation.initial_accumulator_bytes {
            push_field(&mut value, &format!("{bytes:02x?}"));
        }
    }
    value
}

pub(crate) fn canonical_cord(cord: &CheckedCanonicalCord) -> String {
    let mut value = String::new();
    for stage in &cord.stages {
        match stage {
            CheckedCordStage::Reference(reference) => {
                push_field(&mut value, "reference");
                push_field(&mut value, reference);
            }
            CheckedCordStage::RelationalGear {
                operands,
                gear,
                input_ports,
                output_port,
            } => {
                push_field(&mut value, "relational-gear");
                push_field(&mut value, &canonical_gear(gear));
                for (operand, input_port) in operands.iter().zip(input_ports) {
                    push_field(&mut value, operand);
                    push_field(&mut value, input_port);
                }
                push_field(&mut value, output_port);
            }
            CheckedCordStage::TerminalProjection {
                endpoint, terminal, ..
            } => {
                push_field(&mut value, "terminal-projection");
                push_field(&mut value, endpoint);
                push_field(&mut value, &format!("{terminal:?}"));
            }
            CheckedCordStage::Cancellation { gear, .. } => {
                push_field(&mut value, "cancellation");
                push_field(&mut value, gear);
            }
            CheckedCordStage::When { expression, .. } => {
                push_field(&mut value, "when");
                push_field(&mut value, &canonical_expression(expression));
            }
            CheckedCordStage::PureExpression { expression, .. } => {
                push_field(&mut value, "pure-expression");
                push_field(&mut value, &canonical_expression(expression));
            }
            CheckedCordStage::InlineGear(gear) => {
                push_field(&mut value, "inline-gear");
                push_field(&mut value, &canonical_gear(gear));
            }
            CheckedCordStage::Literal { value: literal, .. } => {
                push_field(&mut value, "literal");
                push_field(&mut value, &canonical_value(literal));
            }
            CheckedCordStage::StructuredSelector { selector, .. } => {
                push_field(&mut value, "structured-selector");
                let bytes = selector
                    .canonical_bytes()
                    .expect("a checked selector retains its finite canonical identity");
                let mut encoded = String::with_capacity(bytes.len() * 2);
                push_hex(&mut encoded, &bytes);
                push_field(&mut value, &encoded);
            }
        }
    }
    value
}

pub(crate) fn canonical_expression(expression: &crate::ExpressionSyntax) -> String {
    use crate::ExpressionSyntax;
    let mut value = String::new();
    match expression {
        ExpressionSyntax::Atomic(atomic) => {
            push_field(&mut value, "atomic");
            push_field(&mut value, &atomic.text);
        }
        ExpressionSyntax::Input(_) => push_field(&mut value, "input"),
        ExpressionSyntax::Projection {
            value: projected,
            member,
            ..
        } => {
            push_field(&mut value, "projection");
            push_field(&mut value, &canonical_expression(projected));
            match member {
                crate::ExpressionProjection::Field(field) => {
                    push_field(&mut value, "field");
                    push_field(&mut value, &field.text);
                }
                crate::ExpressionProjection::TupleIndex(index) => {
                    push_field(&mut value, "tuple-index");
                    push_field(&mut value, &index.text);
                }
            }
        }
        ExpressionSyntax::Unary {
            operator, operand, ..
        } => {
            push_field(&mut value, "unary");
            push_field(&mut value, &format!("{operator:?}"));
            push_field(&mut value, &canonical_expression(operand));
        }
        ExpressionSyntax::Binary {
            operator,
            left,
            right,
            ..
        } => {
            push_field(&mut value, "binary");
            push_field(&mut value, &format!("{operator:?}"));
            push_field(&mut value, &canonical_expression(left));
            push_field(&mut value, &canonical_expression(right));
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            push_field(&mut value, "conditional");
            push_field(&mut value, &canonical_expression(condition));
            push_field(&mut value, &canonical_expression(when_true));
            push_field(&mut value, &canonical_expression(when_false));
        }
        ExpressionSyntax::Tuple { values, .. } | ExpressionSyntax::Collection { values, .. } => {
            push_field(
                &mut value,
                if matches!(expression, ExpressionSyntax::Tuple { .. }) {
                    "tuple"
                } else {
                    "collection"
                },
            );
            for item in values {
                push_field(&mut value, &canonical_expression(item));
            }
        }
        ExpressionSyntax::Record { fields, .. } => {
            push_field(&mut value, "record");
            for field in fields {
                push_field(&mut value, &field.name.text);
                push_field(&mut value, &canonical_expression(&field.value));
            }
        }
        ExpressionSyntax::Variant { tag, payload, .. } => {
            push_field(&mut value, "variant");
            push_field(&mut value, &tag.text);
            push_field(&mut value, &canonical_expression(payload));
        }
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => {
            push_field(&mut value, "semantic-call");
            push_field(&mut value, &kind.text);
            for argument in arguments {
                push_field(&mut value, &canonical_expression(argument));
            }
        }
    }
    value
}

fn push_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

fn push_field(target: &mut String, value: &str) {
    target.push_str(&value.len().to_string());
    target.push(':');
    target.push_str(value);
}

pub(crate) fn canonical_value(value: &CanonicalStartupValue) -> String {
    match value {
        CanonicalStartupValue::Literal(value) => format!("literal:{value}"),
        CanonicalStartupValue::Quantity(value) => {
            format!("quantity:{}:{}", value.unit().semantic_id(), value.value())
        }
        CanonicalStartupValue::PlotParameter(name) => format!("parameter:{name}"),
        CanonicalStartupValue::PoolReference(pool) => {
            format!("pool-reference:{}", pool.as_str())
        }
        CanonicalStartupValue::Structured(value) => {
            format!("structured:{}", value.canonical_identity())
        }
    }
}
