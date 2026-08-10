use crate::prelude::*;
use crate::{
    hash_string, CanonicalStartupValue, CheckedCanonicalCord, CheckedCanonicalGear,
    CheckedCordStage, CheckedStartupParameter,
};
use conduit_core::CheckedFormId;

pub(crate) fn checked_identity(
    name: &str,
    parameters: &[CheckedStartupParameter],
    runtime_ports: &[crate::RuntimePort],
    shorthand: Option<(&str, &str)>,
    gears: &[CheckedCanonicalGear],
    cords: &[CheckedCanonicalCord],
    pools: &[crate::CheckedPoolDeclaration],
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
        for parameter in pool.member_face.startup_parameters() {
            push_field(&mut canonical, &parameter.name);
            push_field(&mut canonical, &parameter.value_type);
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
            .member_face
            .inputs()
            .iter()
            .chain(pool.member_face.outputs())
        {
            push_field(&mut canonical, port.port_id.as_str());
            push_field(&mut canonical, port.value_kind.as_str());
            push_field(&mut canonical, port.temporal.as_str());
            push_field(&mut canonical, &format!("{:?}", port.direction));
        }
        if let Some((input, output)) = pool.member_face.shorthand() {
            push_field(&mut canonical, input.as_str());
            push_field(&mut canonical, output.as_str());
        }
    }
    CheckedFormId::from(hash_string(&canonical))
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
            CheckedCordStage::InlineGear(gear) => {
                push_field(&mut value, "inline-gear");
                push_field(&mut value, &canonical_gear(gear));
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
        CanonicalStartupValue::PoolReference(pool) => {
            format!("pool-reference:{}", pool.as_str())
        }
    }
}
