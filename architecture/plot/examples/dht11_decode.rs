//! Decode a finite physical probe transcript with the actual checked Source.
//! This establishes host-side expression evaluation, not MCU kernel execution.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
use serde_json::{json, Value};
use std::{error::Error, io::Read};

trait Diagnostic<T> {
    fn diagnostic(self) -> Result<T, Box<dyn Error>>;
}
impl<T, E: core::fmt::Debug> Diagnostic<T> for Result<T, E> {
    fn diagnostic(self) -> Result<T, Box<dyn Error>> {
        self.map_err(|error| format!("{error:?}").into())
    }
}

fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("checked record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}

fn capture(ty: &StructuredInfoType, wire: &[Value]) -> Result<Vec<u8>, Box<dyn Error>> {
    if wire.len() != 5 {
        return Err("capture must contain exactly five octets".into());
    }
    let wire_type = field_type(ty, "wire");
    let StructuredInfoTypeShape::Collection { element, .. } = wire_type.shape() else {
        panic!("checked octets")
    };
    let mut octets = Vec::with_capacity(5);
    for octet in wire {
        let value = u8::try_from(
            octet
                .as_u64()
                .ok_or("octet is not an unsigned integer")
                .diagnostic()?,
        )
        .diagnostic()?;
        octets.push(StructuredInfoValue::leaf(element.clone(), vec![value]).diagnostic()?);
    }
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "length",
                StructuredInfoValue::leaf(field_type(ty, "length").clone(), vec![5])
                    .diagnostic()?,
            )
            .diagnostic()?,
            StructuredFieldValue::new(
                "wire",
                StructuredInfoValue::collection(wire_type.clone(), octets).diagnostic()?,
            )
            .diagnostic()?,
        ],
    )
    .diagnostic()?
    .canonical_bytes()
    .diagnostic()
}

fn observation(
    raw: StructuredInfoValue,
    checked: &conduit_plot::CheckedSyntaxDocument,
) -> Result<StructuredInfoValue, Box<dyn Error>> {
    let semantic_type = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let StructuredInfoValueShape::Variant { tag, payload } = raw.shape() else {
        return Err("missing decoded result".into());
    };
    if tag == "unavailable" {
        return StructuredInfoValue::variant(
            semantic_type("Dht11Result"),
            "unavailable",
            payload.clone(),
        )
        .diagnostic();
    }
    if tag != "frame" {
        return Err("unknown decoded result".into());
    }
    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
        return Err("missing decoded frame".into());
    };
    let observation = semantic_type("Dht11Observation");
    let mut values = Vec::with_capacity(2);
    for (name, selected) in [
        ("temperature", "device/dht11-temperature"),
        ("humidity", "device/dht11-humidity"),
    ] {
        let field = fields
            .iter()
            .find(|f| f.name() == name)
            .ok_or("missing coordinate")?;
        let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
            return Err("invalid coordinate carrier".into());
        };
        let coefficient = i128::from_le_bytes(bytes.try_into().diagnostic()?);
        let form = checked
            .type_forms
            .iter()
            .find(|f| f.name == selected)
            .ok_or("missing selected Form")?;
        let conduit_plot::CheckedTypeFormStorage::DecimalQuantity(repr) = &form.storage else {
            return Err("wrong selected Form".into());
        };
        let bytes = match repr.integer {
            conduit_core::QuantityFormInteger::I16Le => {
                i16::try_from(coefficient).diagnostic()?.to_le_bytes()
            }
            conduit_core::QuantityFormInteger::U16Le => {
                u16::try_from(coefficient).diagnostic()?.to_le_bytes()
            }
        };
        let quantity = repr.decode(&bytes).diagnostic()?;
        // The checked semantic field contract validates family and point/delta role.
        let value = StructuredInfoValue::leaf(
            field_type(&observation, name).clone(),
            quantity.encode().to_vec(),
        )
        .diagnostic()?;
        values.push(StructuredFieldValue::new(name, value).diagnostic()?);
    }
    let value = StructuredInfoValue::record(observation, values).diagnostic()?;
    StructuredInfoValue::variant(semantic_type("Dht11Result"), "observation", value).diagnostic()
}

fn decoded(value: &StructuredInfoValue) -> Value {
    if let StructuredInfoTypeShape::Leaf(kind) = value.value_type().shape() {
        if conduit_core::quantity_info_dimension(kind.as_str()).is_some()
            || conduit_core::primitive_info_kind(kind.as_str())
                == Some(conduit_core::PrimitiveInfoKind::Quantity)
        {
            let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                unreachable!()
            };
            let quantity = conduit_core::Quantity::decode(bytes).expect("checked physical value");
            return json!({
                "type": quantity.family().role_name(quantity.role()).expect("checked quantity role"),
                "coefficient": quantity.coefficient() as i64,
                "exponent": quantity.exponent(),
                "unit": quantity.unit().symbol(),
            });
        }
    }
    match value.shape() {
        StructuredInfoValueShape::Variant { tag, payload } => json!({tag: decoded(payload)}),
        StructuredInfoValueShape::Record(fields) => Value::Object(
            fields
                .iter()
                .map(|f| (f.name().into(), decoded(f.value())))
                .collect(),
        ),
        StructuredInfoValueShape::Leaf(bytes) if bytes.len() == 16 => {
            json!(i128::from_le_bytes(bytes.try_into().unwrap()) as i64)
        }
        _ => Value::Null,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("expected capture.ndjson path")
        .diagnostic()?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .diagnostic()?
        .take(65_537)
        .read_to_end(&mut bytes)
        .diagnostic()?;
    if bytes.len() > 65_536 {
        return Err("capture exceeds 64 KiB diagnostic bound".into());
    }
    let source = include_str!("../../../plots/device-protocols/dht11.conduit");
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics).into());
    }
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).diagnostic()?;
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "dht11-decode", &ProfileCatalog::new())
            .diagnostic()?
            .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        return Err("missing decoder program".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).diagnostic()?;
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).diagnostic()?;
    let mut count = 0;
    for line in std::str::from_utf8(&bytes)
        .diagnostic()?
        .lines()
        .filter(|line| !line.is_empty())
    {
        count += 1;
        if count > 32 {
            return Err("capture exceeds 32 transactions".into());
        }
        let mut record: Value = serde_json::from_str(line).diagnostic()?;
        if record["schema"] != "conduit.dht11-probe/v1" {
            return Err("unknown capture schema".into());
        }
        record["proof_class"] = json!("host-source-expression-evaluation");
        if record["status"] == "captured" {
            let wire = record["wire"]
                .as_array()
                .ok_or("missing wire octets")
                .diagnostic()?;
            let input = capture(&program.input_type, wire).diagnostic()?;
            let output = evaluator.evaluate(&input).diagnostic()?;
            record["decoded"] = decoded(&observation(
                StructuredInfoValue::from_canonical_bytes(output).diagnostic()?,
                &checked,
            )?);
        }
        println!("{record}");
    }
    if count == 0 {
        return Err("empty transcript establishes no physical observation".into());
    }
    Ok(())
}
