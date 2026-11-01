//! Make-only retention of ordinary checked Look startup values.
use conduit_plot::rust_binding::{validate_native_contracts, validate_native_invariants};
use conduit_plot::{CanonicalStartupValue, CheckedSyntaxDocument};
use std::{env, fs, path::PathBuf};

pub fn generate(checked: &CheckedSyntaxDocument) {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    for name in ["day", "night"] {
        let bytes = resource(checked, name).expect("authored Look contracts must admit");
        fs::write(output.join(format!("field-instrument-{name}.look")), bytes)
            .expect("retain exact checked Look value");
    }
}

/// Syntax/shape checking and Native refinement admission are separate gates.
/// Make runs both before retaining a value; no runtime Source evaluator is added.
pub fn resource(checked: &CheckedSyntaxDocument, name: &str) -> Result<Vec<u8>, String> {
    let look = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "LookInfo")
        .ok_or("canonical Look Type missing")?;
    let plot = checked
        .plots
        .iter()
        .find(|plot| plot.name == format!("look/field-instrument-{name}"))
        .ok_or("authored Look definition missing")?;
    let [parameter] = plot.startup_parameters.as_slice() else {
        return Err("one typed Look value per definition".into());
    };
    if parameter.name != "look" {
        return Err("Look parameter missing".into());
    }
    let Some(CanonicalStartupValue::Structured(value)) = &parameter.default else {
        return Err("Look requires an ordinary concrete checked startup value".into());
    };
    let value = value
        .try_concrete()
        .ok_or("concrete Look resource missing")?;
    if value.value_type() != &look.value_type {
        return Err("foreign Look Type".into());
    }
    validate_native_contracts(&value, &look.value_contracts)
        .map_err(|error| format!("{name} Look refinement: {error:?}"))?;
    validate_native_invariants(&value, &look.invariants)
        .map_err(|error| format!("{name} Look law: {error:?}"))?;
    let bytes = value
        .canonical_bytes()
        .map_err(|error| format!("{name} Look encoding: {error:?}"))?;
    if bytes.len() > 65_536 {
        return Err("existing Look byte admission exceeded".into());
    }
    Ok(bytes)
}
