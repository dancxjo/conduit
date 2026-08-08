use crate::{
    hash_string, CanonicalStartupValue, CheckedCanonicalCell, CheckedCanonicalCord,
    CheckedCordStage, CheckedStartupParameter,
};
use conduit_core::CheckedFormId;

pub(crate) fn checked_identity(
    name: &str,
    parameters: &[CheckedStartupParameter],
    runtime_ports: &[crate::RuntimePort],
    shorthand: Option<(&str, &str)>,
    cells: &[CheckedCanonicalCell],
    cords: &[CheckedCanonicalCord],
) -> CheckedFormId {
    let mut canonical = String::from("canonical-form");
    push_field(&mut canonical, name);
    for parameter in parameters {
        canonical.push_str("param");
        push_field(&mut canonical, &parameter.name);
        push_field(&mut canonical, &parameter.value_type);
        let default = parameter
            .default
            .as_ref()
            .map(canonical_value)
            .unwrap_or_else(|| "required".into());
        push_field(&mut canonical, &default);
    }
    let mut ports = runtime_ports.to_vec();
    ports.sort_by(|left, right| left.name.text.cmp(&right.name.text));
    for port in ports {
        canonical.push_str("port");
        push_field(&mut canonical, &port.name.text);
        push_field(&mut canonical, &port.value_type.text);
        push_field(&mut canonical, &format!("{:?}", port.direction));
        push_field(&mut canonical, &format!("{:?}", port.temporal));
    }
    if let Some((input, output)) = shorthand {
        canonical.push_str("shorthand");
        push_field(&mut canonical, input);
        push_field(&mut canonical, output);
    }
    for cell in cells {
        canonical.push_str("cell");
        push_field(&mut canonical, &canonical_cell(cell));
    }
    for cord in cords {
        canonical.push_str("cord");
        push_field(&mut canonical, &canonical_cord(cord));
    }
    CheckedFormId::from(hash_string(&canonical))
}

pub(crate) fn canonical_cell(cell: &CheckedCanonicalCell) -> String {
    let mut value = String::new();
    push_field(&mut value, cell.name.as_deref().unwrap_or("<anonymous>"));
    push_field(&mut value, &cell.operation);
    for binding in &cell.startup_bindings {
        push_field(&mut value, &binding.name);
        push_field(&mut value, &binding.value_type);
        push_field(&mut value, &canonical_value(&binding.value));
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
            CheckedCordStage::InlineCell(cell) => {
                push_field(&mut value, "inline-cell");
                push_field(&mut value, &canonical_cell(cell));
            }
            CheckedCordStage::Literal { value: literal, .. } => {
                push_field(&mut value, "literal");
                push_field(&mut value, &canonical_value(literal));
            }
        }
    }
    value
}

fn push_field(target: &mut String, value: &str) {
    target.push_str(&value.len().to_string());
    target.push(':');
    target.push_str(value);
}

fn canonical_value(value: &CanonicalStartupValue) -> String {
    match value {
        CanonicalStartupValue::Literal(value) => format!("literal:{value}"),
        CanonicalStartupValue::FormParameter(name) => format!("parameter:{name}"),
    }
}
