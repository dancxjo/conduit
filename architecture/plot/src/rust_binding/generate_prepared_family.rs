//! Exact Source Type closure and opt-in generated metadata projection.
use super::generate::{
    rust_pascal_identifier, rust_snake_identifier, rust_type, unit_type,
    RustBindingGenerationError as Error,
};
use super::generate_options::RustBindingOptions;
use crate::{prelude::*, CheckedNativeType};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, ValueConstraint};
use core::fmt::Write;

pub(super) fn projection<'a>(
    types: &'a [CheckedNativeType],
    options: &RustBindingOptions,
    names: &BTreeMap<String, String>,
) -> Result<Vec<&'a CheckedNativeType>, Error> {
    if options.prepared_family_roots.is_empty() {
        return Ok(Vec::new());
    }
    let owned = types.iter().collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut pending = Vec::new();
    for name in &options.prepared_family_roots {
        let ty = types
            .iter()
            .find(|ty| &ty.name == name)
            .ok_or(Error::InvalidSemanticType)?;
        pending.push(ty);
    }
    while let Some(ty) = pending.pop() {
        if !selected.insert(ty.identity.as_str()) {
            continue;
        }
        if selected.len() > super::MAXIMUM_NATIVE_FAMILY_TYPES {
            return Err(Error::InvalidSemanticType);
        }
        let mut children = Vec::new();
        children_of(&ty.value_type, true, &owned, names, &mut children)?;
        pending.extend(children);
    }
    Ok(types
        .iter()
        .filter(|ty| selected.contains(ty.identity.as_str()))
        .collect())
}

fn children_of<'a>(
    value_type: &StructuredInfoType,
    root: bool,
    types: &[&'a CheckedNativeType],
    names: &BTreeMap<String, String>,
    children: &mut Vec<&'a CheckedNativeType>,
) -> Result<(), Error> {
    let schema = match value_type.shape() {
        StructuredInfoTypeShape::Nominal { schema, .. }
        | StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. } => Some(schema),
        _ => None,
    };
    if !root {
        if let Some(schema) = schema {
            if let Some(ty) = types.iter().find(|ty| &ty.identity == schema) {
                if &ty.value_type != value_type {
                    return Err(Error::InvalidSemanticType);
                }
                if !children.iter().any(|child| child.identity == ty.identity) {
                    children.push(*ty);
                }
                return Ok(());
            }
            if names.contains_key(schema.as_str()) {
                return Err(Error::InvalidSemanticType);
            }
        }
    }
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            children_of(representation, false, types, names, children)?
        }
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => {
            children_of(element, false, types, names, children)?
        }
        StructuredInfoTypeShape::Record { fields, .. } => {
            for field in fields {
                children_of(field.value_type(), false, types, names, children)?;
            }
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            for case in cases {
                children_of(case.payload_type(), false, types, names, children)?;
            }
        }
        StructuredInfoTypeShape::Leaf(_) => {}
    }
    Ok(())
}

pub(super) fn emit(
    out: &mut String,
    selected: &[&CheckedNativeType],
    names: &BTreeMap<String, String>,
    options: &RustBindingOptions,
) -> Result<(), Error> {
    for ty in selected {
        let name = &names[ty.identity.as_str()];
        let constant = super::generate::semantic_constant_name(name)?;
        let mut children = Vec::new();
        // Projection has already verified the entire owned closure. Looking up
        // selected Types repeats exact full-Type checks for each emitted edge.
        children_of(&ty.value_type, true, selected, names, &mut children)?;
        let profile = match ty.value_type.shape() {
            StructuredInfoTypeShape::Record { .. } => "Record",
            StructuredInfoTypeShape::Nominal { .. } => "Nominal",
            StructuredInfoTypeShape::Variant { .. } => "Variant",
            _ => return Err(Error::InvalidSemanticType),
        };
        if profile != "Record" && !ty.invariants.is_empty() {
            return Err(Error::InvalidSemanticType);
        }
        writeln!(out, "#[allow(non_upper_case_globals)]\npub static {name}_PREPARED_NATIVE_DESCRIPTOR: conduit_plot::rust_binding::NativeFamilyTypeDescriptor = conduit_plot::rust_binding::NativeFamilyTypeDescriptor {{\n    type_bytes: {constant},\n    laws: {name}_PREPARED_NATIVE_LAWS,\n    contracts: &[").unwrap();
        for contract in &ty.value_contracts {
            writeln!(out, "        conduit_plot::rust_binding::NativeFamilyContractDescriptor {{ representation_path: {:?}, value_kind: {:?}, maximum_bytes: {}, constraints: &[", contract.representation_path, contract.contract.value_kind.as_str(), contract.contract.maximum_bytes).unwrap();
            for constraint in &contract.contract.constraints {
                let recipe = match constraint {
                    ValueConstraint::CanonicalMembership { members, negated } => format!("CanonicalMembership {{ members: &[{}], negated: {negated} }}", members.iter().map(|bytes| format!("&{bytes:?}")).collect::<Vec<_>>().join(", ")),
                    ValueConstraint::FixedIntegerRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!("FixedIntegerRange {{ minimum: {}, maximum: {}, minimum_endpoint: conduit_core::IntervalEndpoint::{minimum_endpoint:?}, maximum_endpoint: conduit_core::IntervalEndpoint::{maximum_endpoint:?} }}", optional_bytes(minimum), optional_bytes(maximum)),
                    _ => return Err(Error::InvalidSemanticType),
                };
                writeln!(out, "            conduit_plot::rust_binding::NativeFamilyConstraintDescriptor::{recipe},").unwrap();
            }
            writeln!(out, "        ] }},").unwrap();
        }
        writeln!(out, "    ],\n    children: &[{}],\n    conversion_profile: conduit_plot::rust_binding::NativeFamilyConversionProfile::{profile},\n    maximum_inline_bytes: {},\n}};", children.iter().map(|child| format!("&{}_PREPARED_NATIVE_DESCRIPTOR", names[child.identity.as_str()])).collect::<Vec<_>>().join(", "), layout_bound(ty, name, names)?).unwrap();
        emit_converter(out, ty, name, names, options)?;
    }
    writeln!(out, "pub static PREPARED_NATIVE_FAMILY_ROOTS: &[&conduit_plot::rust_binding::NativeFamilyTypeDescriptor] = &[{}];", selected.iter().filter(|ty| options.prepared_family_roots.contains(&ty.name)).map(|ty| format!("&{}_PREPARED_NATIVE_DESCRIPTOR", names[ty.identity.as_str()])).collect::<Vec<_>>().join(", ")).unwrap();
    Ok(())
}

fn optional_bytes(bytes: &Option<Vec<u8>>) -> String {
    bytes
        .as_ref()
        .map_or_else(|| "None".into(), |bytes| format!("Some(&{bytes:?})"))
}

fn layout_bound(
    ty: &CheckedNativeType,
    name: &str,
    names: &BTreeMap<String, String>,
) -> Result<String, Error> {
    let mut layouts = vec![format!("core::mem::size_of::<{name}>()")];
    if let StructuredInfoTypeShape::Variant { cases, .. } = ty.value_type.shape() {
        for case in cases {
            if matches!(
                case.payload_type().shape(),
                StructuredInfoTypeShape::Record { .. }
            ) {
                layouts.push(format!(
                    "core::mem::size_of::<{name}{}>()",
                    rust_pascal_identifier(case.tag())?
                ));
            }
        }
    }
    fn walk(
        ty: &StructuredInfoType,
        names: &BTreeMap<String, String>,
        layouts: &mut Vec<String>,
    ) -> Result<(), Error> {
        match ty.shape() {
            StructuredInfoTypeShape::Collection { element, .. }
            | StructuredInfoTypeShape::Sequence { element, .. } => {
                layouts.push(format!(
                    "core::mem::size_of::<{}>()",
                    rust_type(element, names)?
                ));
                walk(element, names, layouts)?;
            }
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                walk(representation, names, layouts)?
            }
            StructuredInfoTypeShape::Record { fields, .. } => {
                for field in fields {
                    walk(field.value_type(), names, layouts)?;
                }
            }
            StructuredInfoTypeShape::Variant { cases, .. } => {
                for case in cases {
                    walk(case.payload_type(), names, layouts)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(&ty.value_type, names, &mut layouts)?;
    if layouts.len() == 1 {
        return Ok(layouts.remove(0));
    }
    Ok(format!(
        "{{ let mut largest = {}; {} largest }}",
        layouts[0],
        layouts[1..]
            .iter()
            .map(|layout| format!("if {layout} > largest {{ largest = {layout}; }}"))
            .collect::<Vec<_>>()
            .join(" ")
    ))
}

fn emit_converter(
    out: &mut String,
    ty: &CheckedNativeType,
    name: &str,
    names: &BTreeMap<String, String>,
    options: &RustBindingOptions,
) -> Result<(), Error> {
    writeln!(out, "impl conduit_plot::rust_binding::PreparedNativeRustBinding for {name} {{\n    const PREPARED_DESCRIPTOR: &'static conduit_plot::rust_binding::NativeFamilyTypeDescriptor = &{name}_PREPARED_NATIVE_DESCRIPTOR;\n    fn from_borrowed_prepared(value: conduit_core::ValidatedCanonicalStructuredValue<'_>, family: &mut conduit_plot::rust_binding::PreparedNativeFamily) -> Result<Self, NativeBindingRefusal> {{\n        family.check_type(Self::PREPARED_DESCRIPTOR, value)?;").unwrap();
    match ty.value_type.shape() {
        StructuredInfoTypeShape::Record { fields, .. } => {
            let order = options.record_constructor_orders.get(&ty.name);
            let fields = order
                .map(|order| {
                    order
                        .iter()
                        .map(|name| {
                            fields
                                .iter()
                                .find(|field| field.name() == name)
                                .ok_or(Error::InvalidSemanticType)
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_else(|| fields.iter().collect());
            for (index, field) in fields.iter().enumerate() {
                writeln!(out, "        let __conduit_prepared_field_{index} = {};", decode(field.value_type(), &format!("value.record_field({:?}).map_err(NativeBindingRefusal::InvalidValue)?.ok_or(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?", field.name()), names)?).unwrap();
            }
            writeln!(out, "        family.validate(Self::PREPARED_DESCRIPTOR, value)?;\n        Ok(Self {{ {} }})", fields.iter().enumerate().map(|(index, field)| Ok(format!("{}: __conduit_prepared_field_{index}", rust_snake_identifier(field.name())?))).collect::<Result<Vec<_>, Error>>()?.join(", ")).unwrap();
        }
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            writeln!(out, "        let decoded = {};\n        family.validate(Self::PREPARED_DESCRIPTOR, value)?;\n        Ok(Self(decoded))", decode(representation, "value.nominal_representation().map_err(NativeBindingRefusal::InvalidValue)?", names)?).unwrap();
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            writeln!(
                out,
                "        match value.variant_tag().map_err(NativeBindingRefusal::InvalidValue)? {{"
            )
            .unwrap();
            for case in cases {
                let variant = rust_pascal_identifier(case.tag())?;
                let boxed =
                    options
                        .boxed_variant_payloads
                        .contains(&format!("{}.{}", ty.name, case.tag()));
                let payload = format!("value.variant_payload({:?}).map_err(NativeBindingRefusal::InvalidValue)?.ok_or(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?", case.tag());
                let result = if unit_type(case.payload_type()) {
                    format!("Self::{variant}")
                } else if let StructuredInfoTypeShape::Record { fields, .. } =
                    case.payload_type().shape()
                {
                    let fields = fields.iter().map(|field| Ok(format!("{}: {}", rust_snake_identifier(field.name())?, decode(field.value_type(), &format!("payload.record_field({:?}).map_err(NativeBindingRefusal::InvalidValue)?.ok_or(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?", field.name()), names)?))).collect::<Result<Vec<_>, Error>>()?.join(", ");
                    format!(
                        "Self::{variant}({}{name}{variant} {{ {fields} }}{})",
                        if boxed { "Box::new(" } else { "" },
                        if boxed { ")" } else { "" }
                    )
                } else {
                    format!(
                        "Self::{variant}({}{}{})",
                        if boxed { "Box::new(" } else { "" },
                        decode(case.payload_type(), "payload", names)?,
                        if boxed { ")" } else { "" }
                    )
                };
                writeln!(out, "            {:?} => {{ let payload = {payload}; let _ = payload; Ok({result}) }},", case.tag()).unwrap();
            }
            writeln!(out, "            _ => Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)),\n        }}").unwrap();
        }
        _ => return Err(Error::InvalidSemanticType),
    }
    writeln!(out, "    }}\n}}\n").unwrap();
    Ok(())
}

fn decode(
    ty: &StructuredInfoType,
    value: &str,
    names: &BTreeMap<String, String>,
) -> Result<String, Error> {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { schema, .. } | StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. } if names.contains_key(schema.as_str()) =>
            Ok(format!("<{} as conduit_plot::rust_binding::PreparedNativeRustBinding>::from_borrowed_prepared({value}, family)?", names[schema.as_str()])),
        StructuredInfoTypeShape::Leaf(kind) => {
            let rust_type = super::generate::primitive_rust_type(kind.as_str())?;
            Ok(format!("{{ let leaf = {value}; let (_, bytes) = leaf.primitive().map_err(NativeBindingRefusal::InvalidValue)?; <{rust_type} as conduit_plot::rust_binding::NativePrimitive>::decode_primitive(bytes).ok_or(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))? }}"))
        }
        StructuredInfoTypeShape::Sequence { element, maximum_items, .. } => Ok(format!("{{ let sequence = {value}; let mut result = BoundedSequence::<{}, {maximum_items}>::new(); for item in sequence.collection_elements().map_err(NativeBindingRefusal::InvalidValue)? {{ let item = item.map_err(NativeBindingRefusal::InvalidValue)?; result.push({}).map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongCollectionLength))?; }} result }}", rust_type(element, names)?, decode(element, "item", names)?)),
        StructuredInfoTypeShape::Collection { element, length } => Ok(format!("{{ let collection = {value}; let mut result = Vec::with_capacity({length}); for item in collection.collection_elements().map_err(NativeBindingRefusal::InvalidValue)? {{ let item = item.map_err(NativeBindingRefusal::InvalidValue)?; result.push({}); }} let result: [{}; {length}] = result.try_into().map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongCollectionLength))?; result }}", decode(element, "item", names)?, rust_type(element, names)?)),
        StructuredInfoTypeShape::Variant { schema, cases } if schema.as_str() == "conduit.conduitese.optional.v1" => {
            let some = cases.iter().find(|case| case.tag() == "some").ok_or(Error::InvalidSemanticType)?;
            Ok(format!("{{ let optional = {value}; match optional.variant_payload(\"some\").map_err(NativeBindingRefusal::InvalidValue)? {{ Some(payload) => Some({}), None => None }} }}", decode(some.payload_type(), "payload", names)?))
        }
        _ => Err(Error::InvalidSemanticType),
    }
}
