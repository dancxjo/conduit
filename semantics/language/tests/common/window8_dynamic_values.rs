use conduit_core::*;
use conduit_plot::*;
pub fn record(
    ty: &conduit_plot::CheckedNativeType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.value_type.clone(),
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
pub fn native<'a>(
    checked: &'a conduit_plot::CheckedSyntaxDocument,
    name: &str,
) -> &'a conduit_plot::CheckedNativeType {
    checked
        .native_types
        .iter()
        .find(|ty| ty.name == name)
        .unwrap()
}

pub fn field_type(ty: &CheckedNativeType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.value_type.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
pub fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .clone()
}
pub fn scalar(ty: &CheckedNativeType, name: &str, value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(field_type(ty, name), value.to_le_bytes().to_vec()).unwrap()
}
pub fn checked_program(checked: &CheckedSyntaxDocument, entry: &str) -> PortableExpressionProgram {
    let expanded =
        expand_canonical_plot_for_authoring(checked, entry, &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
pub fn replace(
    value: &StructuredInfoValue,
    name: &str,
    next: StructuredInfoValue,
) -> StructuredInfoValue {
    let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        value.value_type().clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    if f.name() == name {
                        next.clone()
                    } else {
                        f.value().clone()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
