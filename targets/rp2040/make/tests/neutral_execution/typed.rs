//! Cold typed fixture construction; no schemas or values come from the command.
use conduit_core as c;
use conduit_plot as p;
use std::fmt::Write;
fn leaf(schema: &c::StructuredInfoType, byte: u8) -> c::StructuredInfoValue {
    c::StructuredInfoValue::leaf(schema.clone(), vec![byte]).unwrap()
}
fn record(schema: &c::StructuredInfoType, values: &[(&str, u8)]) -> c::StructuredInfoValue {
    let c::StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        panic!("record")
    };
    assert_eq!(fields.len(), values.len());
    c::StructuredInfoValue::record(
        schema.clone(),
        fields
            .iter()
            .map(|field| {
                let value = values
                    .iter()
                    .find(|(name, _)| *name == field.name())
                    .unwrap()
                    .1;
                c::StructuredFieldValue::new(field.name(), leaf(field.value_type(), value)).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

pub fn emit(fixture: &super::environmental_exchange_source::Fixture) -> String {
    let mut startup = p::StartupCatalog::new();
    let mut profiles = p::ProfileCatalog::new();
    super::source_build::protocol_catalog::install(&mut startup, &mut profiles).unwrap();
    let types =
        p::prepare_source_types(&p::parse_syntax_document(&fixture.source), &startup).unwrap();
    let workset = fixture.compile().unwrap();
    let front = &workset.entries[0].plot.front;
    let ports = [
        ("begin", "BmeProtocolBegin"),
        ("configuration", "Ht16k33Configuration"),
        ("frame", "Ht16k33Frame"),
        ("captured", "BmeProtocolState"),
        ("observation", "BmeObservationResult"),
        ("sensor_refused", "BmeProtocolFailure"),
        ("clock_failed", "ClockResult"),
        ("initialized", "Ht16k33Acknowledgement"),
        ("initialization_refused", "Ht16k33Failure"),
        ("written", "Ht16k33Acknowledgement"),
        ("write_refused", "Ht16k33Failure"),
        ("unrouted", "I2cCorrelatedResult"),
    ];
    assert_eq!((front.inputs().len(), front.outputs().len()), (3, 9));
    let mut out = String::from("mod typed {pub static PORTS: [(&str,&[u8]);12]=[");
    for (name, kind) in ports {
        let schema = types.named_type(kind).unwrap();
        let port = front
            .inputs()
            .iter()
            .chain(front.outputs())
            .find(|port| port.port_id.as_str() == name)
            .unwrap();
        assert_eq!(&port.value_kind, schema.profile().unwrap().value_kind());
        writeln!(out, "({name:?},&{:?}),", schema.canonical_bytes().unwrap()).unwrap();
    }
    out.push_str("];\n");
    let begin = record(
        types.named_type("BmeProtocolBegin").unwrap(),
        &[("address", 0x76)],
    );
    let configuration = record(
        types.named_type("Ht16k33Configuration").unwrap(),
        &[
            ("address", 0x70),
            ("brightness", 7),
            ("blink", 0),
            ("enabled", 1),
        ],
    );
    let schema = types.named_type("Ht16k33Frame").unwrap();
    let c::StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        panic!("frame")
    };
    let c::StructuredInfoTypeShape::Collection { element, .. } = fields[1].value_type().shape()
    else {
        panic!("ram")
    };
    let frame = c::StructuredInfoValue::record(
        schema.clone(),
        vec![
            c::StructuredFieldValue::new(fields[0].name(), leaf(fields[0].value_type(), 0x70))
                .unwrap(),
            c::StructuredFieldValue::new(
                fields[1].name(),
                c::StructuredInfoValue::collection(
                    fields[1].value_type().clone(),
                    (0..16).map(|i| leaf(element, i)).collect(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    writeln!(
        out,
        "pub static INPUTS:[&[u8];3]=[&{:?},&{:?},&{:?}];}}",
        begin.canonical_bytes().unwrap(),
        configuration.canonical_bytes().unwrap(),
        frame.canonical_bytes().unwrap()
    )
    .unwrap();
    out
}
