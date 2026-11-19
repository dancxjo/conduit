//! Lower the same checked Look used by general Masks into fixed native constants.
use super::{bounded_bytes, checked_resource, CheckedSyntaxDocument, StructuredInfoValue};
use conduit_plot::rust_binding::{
    primitive_from_structured, semantic_core::StructuredInfoValueShape,
};
use std::{fmt::Write, fs, path::Path};

fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected Look record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .expect("checked Look field")
        .value()
}
fn case(value: &StructuredInfoValue) -> (&str, &StructuredInfoValue) {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("expected Look case")
    };
    (tag, payload)
}

pub(super) fn generate(checked: &CheckedSyntaxDocument, output: &Path) {
    let look = checked_resource(checked, "look/picture-frame").expect("checked picture-frame Look");
    fs::write(
        output.join("picture-frame.look"),
        bounded_bytes(&look).unwrap(),
    )
    .unwrap();
    let (tag, visual) = case(field(&look, "visual"));
    assert_eq!(tag, "some");
    let expr = field(visual, "expression");
    let tokens = field(visual, "tokens");
    let id: String = primitive_from_structured(field(&look, "identity")).unwrap();
    assert_eq!(
        primitive_from_structured::<String>(field(expr, "identity")).unwrap(),
        id
    );
    let mut source = format!("pub const PICTURE_FRAME_LOOK_ID: &str = {id:?};\n");
    let scale: u16 = primitive_from_structured(field(expr, "text_scale_percent")).unwrap();
    writeln!(source, "const SCALE:u16={scale};").unwrap();
    for (name, allowed, ty) in [
        ("density", &["compact", "spacious"][..], "VisualDensity"),
        ("motion", &["none", "restrained"][..], "VisualMotion"),
        ("palette", &["phosphor", "editorial"][..], "VisualPalette"),
        (
            "typography",
            &["editorial", "engineering"][..],
            "VisualTypography",
        ),
    ] {
        let variant = case(field(expr, name)).0;
        assert!(allowed.contains(&variant));
        let variant = format!("{}{}", variant[..1].to_uppercase(), &variant[1..]);
        writeln!(
            source,
            "const {}:{ty}={ty}::{variant};",
            name.to_uppercase()
        )
        .unwrap();
    }
    for name in ["type_body_px", "type_small_px", "line_height_percent"] {
        let n: u16 = primitive_from_structured(field(tokens, name)).unwrap();
        writeln!(source, "const {}:u16={n};", name.to_uppercase()).unwrap();
    }
    for name in [
        "background",
        "surface",
        "text_primary",
        "text_secondary",
        "emphasis",
        "warning",
        "success",
        "failure",
        "focus",
        "structure_primary",
    ] {
        let rgb = ["red", "green", "blue"]
            .map(|c| primitive_from_structured::<u8>(field(field(tokens, name), c)).unwrap());
        writeln!(source, "const {}:[u8;3]={rgb:?};", name.to_uppercase()).unwrap();
    }
    fs::write(output.join("picture-frame-style.rs"), source).unwrap();
}
