//! Checked Source-only bounded reentry policy. No native/device proof or
//! independent storage authority is synthesized by these evaluator fixtures.
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
use conduitos::protocol_source::*;

pub(super) fn source() -> PreparedProtocolSource {
    source_with("")
}
pub(super) fn source_with(extra: &str) -> PreparedProtocolSource {
    let specifications: Vec<ProtocolSpecializationRequest> = serde_json::from_str(include_str!(
        "../../plots/usb/mass-storage-read-reentry-specializations.json"
    ))
    .unwrap();
    let text = format!(
        "{}\n{}",
        include_str!("../../plots/usb/mass-storage-read-reentry.conduit"),
        extra
    );
    let package = ProtocolSourcePackage::compile(text, &specifications).unwrap();
    PreparedProtocolSource::prepare(package).unwrap()
}
pub(super) fn program(source: &PreparedProtocolSource, name: &str) -> PortableExpressionProgram {
    let expanded = source
        .expand(name)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("expression")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
pub(super) fn fields(ty: &StructuredInfoType) -> &[StructuredFieldType] {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
}
pub(super) fn record(
    ty: &StructuredInfoType,
    mut make: impl FnMut(&StructuredFieldType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        fields(ty)
            .iter()
            .map(|f| StructuredFieldValue::new(f.name(), make(f)).unwrap())
            .collect(),
    )
    .unwrap()
}
pub(super) fn scalar(ty: &StructuredInfoType, n: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), n.to_le_bytes().to_vec()).unwrap()
}
pub(super) fn boolean(ty: &StructuredInfoType, b: bool) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), vec![u8::from(b)]).unwrap()
}
pub(super) fn variant(
    ty: &StructuredInfoType,
    tag: &str,
    make: impl FnOnce(&StructuredInfoType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let payload = cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type();
    StructuredInfoValue::variant(ty.clone(), tag, make(payload)).unwrap()
}
pub(super) fn unit(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    variant(ty, tag, |p| {
        StructuredInfoValue::leaf(p.clone(), vec![]).unwrap()
    })
}
pub(super) fn identity(ty: &StructuredInfoType, ordinal: u64) -> StructuredInfoValue {
    record(ty, |f| {
        scalar(
            f.value_type(),
            match f.name() {
                "block_size" => 512,
                "count" => 1,
                "lba" => 0,
                "ordinal" => ordinal,
                _ => panic!("identity"),
            },
        )
    })
}
pub(super) fn read_completed(
    ty: &StructuredInfoType,
    wire: &[u8],
    ordinal: u64,
    short: bool,
) -> StructuredInfoValue {
    variant(ty, "completed", |p| {
        record(p, |f| match f.name() {
            "actual" => scalar(f.value_type(), wire.len() as u64),
            "ordinal" => scalar(f.value_type(), ordinal),
            "short" => boolean(f.value_type(), short),
            "wire" => StructuredInfoValue::leaf(f.value_type().clone(), wire.to_vec()).unwrap(),
            _ => panic!("read field"),
        })
    })
}
pub(super) fn state(ty: &StructuredInfoType, phase: u64) -> StructuredInfoValue {
    let text = include_str!("../../plots/usb/mass-storage-read-reentry.conduit");
    let bank = conduit_data::bytes_literal::decode_hex(
        text.split("bytes/literal(\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap(),
    )
    .unwrap();
    record(ty, |f| match f.name() {
        "bank" => StructuredInfoValue::leaf(f.value_type().clone(), bank.clone()).unwrap(),
        "request" | "permit" => identity(f.value_type(), 0),
        "data" => {
            if phase >= 14 {
                read_completed(f.value_type(), &[0x80; 512], 1, false)
            } else {
                unit(f.value_type(), "unsupported")
            }
        }
        "phase" => scalar(f.value_type(), phase),
        "failed" | "terminal" => boolean(f.value_type(), false),
        _ => panic!("state field"),
    })
}
pub(super) fn pair(
    ty: &StructuredInfoType,
    phase: u64,
    event: StructuredInfoValue,
) -> StructuredInfoValue {
    let members = fields(ty);
    assert_eq!(members.len(), 2);
    assert_eq!(
        ty,
        &tuple_info_type(
            members
                .iter()
                .map(|field| field.value_type().clone())
                .collect()
        )
        .unwrap()
    );
    record(ty, |f| {
        if f.name() == members[0].name() {
            state(f.value_type(), phase)
        } else {
            assert_eq!(f.value_type(), event.value_type());
            event.clone()
        }
    })
}
pub(super) fn evaluate(
    program: &PortableExpressionProgram,
    input: &StructuredInfoValue,
) -> StructuredInfoValue {
    let bytes = input.canonical_bytes().unwrap();
    let expected = program.evaluate(&bytes).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(program).unwrap();
    assert_eq!(prepared.evaluate(&bytes).unwrap(), expected);
    StructuredInfoValue::from_canonical_bytes(&expected).unwrap()
}
pub(super) fn accepted(
    program: &PortableExpressionProgram,
    phase: u64,
    event: StructuredInfoValue,
) -> bool {
    let output = evaluate(program, &pair(&program.input_type, phase, event));
    let bytes = output.canonical_bytes().unwrap();
    validate_canonical_structured_value(&bytes)
        .unwrap()
        .record_field("accepted")
        .unwrap()
        .unwrap()
        .primitive_bytes("value/bool")
        .unwrap()
        == [1]
}
pub(super) fn read_event(
    source: &PreparedProtocolSource,
    event_type: &StructuredInfoType,
    wire: Option<&[u8]>,
    ordinal: u64,
    short: bool,
) -> StructuredInfoValue {
    let facts = program(source, "bot-reentry-read-facts");
    let input = wire.map_or_else(
        || unit(&facts.input_type, "stalled"),
        |wire| read_completed(&facts.input_type, wire, ordinal, short),
    );
    let output = evaluate(&facts, &input);
    variant(event_type, "read", |_| output)
}

pub(super) fn settled_event(
    source: &PreparedProtocolSource,
    completed: bool,
) -> StructuredInfoValue {
    let facts = program(source, "bot-reentry-settled");
    let result = if completed {
        variant(&facts.input_type, "completed", |observation| {
            record(observation, |f| match f.name() {
                "bytes" => {
                    StructuredInfoValue::leaf(f.value_type().clone(), vec![0x80; 512]).unwrap()
                }
                "short" => boolean(f.value_type(), false),
                "actual" | "block_size" => scalar(f.value_type(), 512),
                "count" => scalar(f.value_type(), 1),
                "lba" | "ordinal" | "residue" => scalar(f.value_type(), 0),
                _ => panic!("observation"),
            })
        })
    } else {
        variant(&facts.input_type, "stalled", |id| identity(id, 0))
    };
    evaluate(&facts, &result)
}
