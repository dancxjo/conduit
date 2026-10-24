//! Execute the authored packet expression during fixture preparation. The
//! portable driver has no dependency on the environmental appliance or its Mask.
use conduit_core as c;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
pub(super) fn encoded() -> Vec<u8> {
    let syntax = parse_syntax_document(include_str!(
        "../../../../../plots/device-protocols/tm1637.conduit"
    ));
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "tm1637-packets",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    let c::ConfigurationValue::Text(program) = &expanded.gears[0].configuration[0].value else {
        panic!("checked packet expression");
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let mut registers = [0x7d, 0x87, 0x7d, 0x71, 0, 0].into_iter();
    let input = frame(&program.input_type, "frame", &mut registers)
        .canonical_bytes()
        .unwrap();
    let result =
        c::StructuredInfoValue::from_canonical_bytes(&program.evaluate(&input).unwrap()).unwrap();
    let c::StructuredInfoValueShape::Variant {
        tag: "packets",
        payload,
    } = result.shape()
    else {
        panic!("Source packet result");
    };
    payload.canonical_bytes().unwrap()
}
fn frame(
    ty: &c::StructuredInfoType,
    name: &str,
    registers: &mut impl Iterator<Item = u8>,
) -> c::StructuredInfoValue {
    use c::{StructuredInfoTypeShape as T, StructuredInfoValue as V};
    match ty.shape() {
        T::Leaf(kind) => V::leaf(
            ty.clone(),
            match kind.as_str() {
                c::BOOL_INFO_ID => c::InfoBool::new(true).encode().to_vec(),
                "value/u8" => vec![if name == "brightness" {
                    3
                } else {
                    registers.next().unwrap()
                }],
                _ => panic!("unexpected frame leaf"),
            },
        )
        .unwrap(),
        T::Collection { element, length } => V::collection(
            ty.clone(),
            (0..length)
                .map(|_| frame(element, name, registers))
                .collect(),
        )
        .unwrap(),
        T::Record { fields, .. } => V::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    c::StructuredFieldValue::new(
                        field.name(),
                        frame(field.value_type(), field.name(), registers),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        _ => panic!("unexpected frame shape"),
    }
}
