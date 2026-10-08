//! Exact Source Type closure and opt-in generated metadata projection.
use super::generate::ExternalPreparedNativeRustBinding;
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
    external_prepared: &[ExternalPreparedNativeRustBinding<'_>],
) -> Result<Vec<&'a CheckedNativeType>, Error> {
    if options.prepared_family_roots.is_empty() {
        return Ok(Vec::new());
    }
    let owned = types.iter().collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    for name in &options.prepared_family_roots {
        let root = types
            .iter()
            .find(|ty| &ty.name == name)
            .ok_or(Error::InvalidSemanticType)?;
        // A shared generated module may serve several finite owners. Check each
        // complete root closure independently: no owner can split a root's child
        // validation across families or borrow another owner's descriptor bank.
        let mut root_selected = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(ty) = pending.pop() {
            if !root_selected.insert(ty.identity.as_str()) {
                continue;
            }
            if root_selected.len() > super::MAXIMUM_NATIVE_FAMILY_TYPES {
                return Err(Error::InvalidSemanticType);
            }
            selected.insert(ty.identity.as_str());
            if selected.len() > super::MAXIMUM_GENERATED_NATIVE_FAMILY_TYPES {
                return Err(Error::InvalidSemanticType);
            }
            let mut children = Vec::new();
            let mut external_children = Vec::new();
            children_of(
                &ty.value_type,
                true,
                &owned,
                names,
                &mut children,
                external_prepared,
                &mut external_children,
            )?;
            for external in external_children {
                include_external(external.descriptor, &mut root_selected, &mut selected)?;
            }
            pending.extend(children);
        }
    }
    Ok(types
        .iter()
        .filter(|ty| selected.contains(ty.identity.as_str()))
        .collect())
}

fn children_of<'a, 'b>(
    value_type: &StructuredInfoType,
    root: bool,
    types: &[&'a CheckedNativeType],
    names: &BTreeMap<String, String>,
    children: &mut Vec<&'a CheckedNativeType>,
    external_prepared: &'b [ExternalPreparedNativeRustBinding<'b>],
    external_children: &mut Vec<&'b ExternalPreparedNativeRustBinding<'b>>,
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
                let external = external_prepared
                    .iter()
                    .find(|binding| binding.semantic_identity == schema.as_str())
                    .ok_or(Error::InvalidSemanticType)?;
                if value_type
                    .canonical_bytes()
                    .map_err(|_| Error::InvalidSemanticType)?
                    != external.descriptor.type_bytes
                {
                    return Err(Error::InvalidSemanticType);
                }
                if !external_children
                    .iter()
                    .any(|child| child.semantic_identity == external.semantic_identity)
                {
                    external_children.push(external);
                }
                return Ok(());
            }
        }
    }
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => children_of(
            representation,
            false,
            types,
            names,
            children,
            external_prepared,
            external_children,
        )?,
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => children_of(
            element,
            false,
            types,
            names,
            children,
            external_prepared,
            external_children,
        )?,
        StructuredInfoTypeShape::Record { fields, .. } => {
            for field in fields {
                children_of(
                    field.value_type(),
                    false,
                    types,
                    names,
                    children,
                    external_prepared,
                    external_children,
                )?;
            }
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            for case in cases {
                children_of(
                    case.payload_type(),
                    false,
                    types,
                    names,
                    children,
                    external_prepared,
                    external_children,
                )?;
            }
        }
        StructuredInfoTypeShape::Leaf(_) => {}
    }
    Ok(())
}

fn include_external<'a>(
    descriptor: &'static super::NativeFamilyTypeDescriptor,
    root: &mut BTreeSet<&'a str>,
    selected: &mut BTreeSet<&'a str>,
) -> Result<(), Error> {
    super::prepared_family_external::metadata_extent(descriptor)
        .map_err(|_| Error::InvalidSemanticType)?;
    // Count identities without retaining decoded Types: their original canonical
    // schema strings are borrowed directly from the static canonical frame below.
    let mut seen = Vec::new();
    let mut pending = vec![descriptor];
    while let Some(descriptor) = pending.pop() {
        if seen.iter().any(|prior| core::ptr::eq(*prior, descriptor)) {
            continue;
        }
        if seen.len() >= super::MAXIMUM_NATIVE_FAMILY_TYPES
            || descriptor.children.len() > super::MAXIMUM_NATIVE_FAMILY_TYPES
            || descriptor.type_bytes.len() > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(Error::InvalidSemanticType);
        }
        seen.push(descriptor);
        let value_type = StructuredInfoType::from_canonical_bytes(descriptor.type_bytes)
            .map_err(|_| Error::InvalidSemanticType)?;
        let schema = match value_type.shape() {
            StructuredInfoTypeShape::Nominal { schema, .. }
            | StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. } => schema.as_str(),
            _ => return Err(Error::InvalidSemanticType),
        };
        // The canonical Type stores exactly this validated schema spelling. Locate
        // that byte slice in its immutable frame rather than leaking decoded data.
        let encoded = descriptor
            .type_bytes
            .windows(schema.len())
            .position(|bytes| bytes == schema.as_bytes())
            .ok_or(Error::InvalidSemanticType)?;
        let identity =
            core::str::from_utf8(&descriptor.type_bytes[encoded..encoded + schema.len()])
                .map_err(|_| Error::InvalidSemanticType)?;
        root.insert(identity);
        selected.insert(identity);
        if root.len() > super::MAXIMUM_NATIVE_FAMILY_TYPES
            || selected.len() > super::MAXIMUM_GENERATED_NATIVE_FAMILY_TYPES
        {
            return Err(Error::InvalidSemanticType);
        }
        pending.extend_from_slice(descriptor.children);
    }
    Ok(())
}

pub(super) fn emit(
    out: &mut String,
    selected: &[&CheckedNativeType],
    names: &BTreeMap<String, String>,
    options: &RustBindingOptions,
    external_prepared: &[ExternalPreparedNativeRustBinding<'_>],
) -> Result<(), Error> {
    for ty in selected {
        let name = &names[ty.identity.as_str()];
        let constant = super::generate::semantic_constant_name(name)?;
        let mut children = Vec::new();
        // Projection has already verified the entire owned closure. Looking up
        // selected Types repeats exact full-Type checks for each emitted edge.
        let mut external_children = Vec::new();
        children_of(
            &ty.value_type,
            true,
            selected,
            names,
            &mut children,
            external_prepared,
            &mut external_children,
        )?;
        let mut external_edges = Vec::new();
        for (index, external) in external_children.iter().enumerate() {
            let expected = emit_external_shadow(
                out,
                external.descriptor,
                &format!("{name}_EXTERNAL_{index}"),
            )?;
            external_edges.push(format!("conduit_plot::rust_binding::NativeFamilyExternalEdge {{ descriptor: <{} as conduit_plot::rust_binding::PreparedNativeRustBinding>::PREPARED_DESCRIPTOR, expected: &{expected} }}", external.rust_type_path));
        }
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
        writeln!(out, "    ],\n    children: &[{}],\n    external_edges: &[{}],\n    conversion_profile: conduit_plot::rust_binding::NativeFamilyConversionProfile::{profile},\n    maximum_inline_bytes: {},\n}};", children.iter().map(|child| format!("&{}_PREPARED_NATIVE_DESCRIPTOR", names[child.identity.as_str()])).chain(external_children.iter().map(|child| format!("<{} as conduit_plot::rust_binding::PreparedNativeRustBinding>::PREPARED_DESCRIPTOR", child.rust_type_path))).collect::<Vec<_>>().join(", "), external_edges.join(", "), layout_bound(ty, name, names)?).unwrap();
        emit_converter(out, &ty.value_type, &ty.name, name, names, options)?;
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
        "{{
        let mut largest = {};
        {}
        largest
    }}",
        layouts[0],
        layouts[1..]
            .iter()
            .map(|layout| format!("if {layout} > largest {{ largest = {layout}; }}"))
            .collect::<Vec<_>>()
            .join("\n        ")
    ))
}

pub(super) fn emit_converter(
    out: &mut String,
    value_type: &StructuredInfoType,
    authored_name: &str,
    name: &str,
    names: &BTreeMap<String, String>,
    options: &RustBindingOptions,
) -> Result<(), Error> {
    let destination = out;
    let mut generated = String::new();
    let out = &mut generated;
    writeln!(out, "impl conduit_plot::rust_binding::PreparedNativeRustBinding for {name} {{\n    const PREPARED_DESCRIPTOR: &'static conduit_plot::rust_binding::NativeFamilyTypeDescriptor = &{name}_PREPARED_NATIVE_DESCRIPTOR;\n    fn from_borrowed_prepared(value: conduit_core::ValidatedCanonicalStructuredValue<'_>, family: &mut conduit_plot::rust_binding::PreparedNativeFamily) -> Result<Self, NativeBindingRefusal> {{\n        family.check_type(Self::PREPARED_DESCRIPTOR, value)?;").unwrap();
    match value_type.shape() {
        StructuredInfoTypeShape::Record { fields, .. } => {
            let order = options.record_constructor_orders.get(authored_name);
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
                writeln!(out, "        let __conduit_prepared_field_{index} = {};", decode(field.value_type(), &format!("family.record_field(Self::PREPARED_DESCRIPTOR, value, {:?})?.ok_or(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?", field.name()), names)?).unwrap();
            }
            writeln!(out, "        family.validate(Self::PREPARED_DESCRIPTOR, value)?;\n        Ok(Self {{ {} }})", fields.iter().enumerate().map(|(index, field)| Ok(format!("{}: __conduit_prepared_field_{index}", rust_snake_identifier(field.name())?))).collect::<Result<Vec<_>, Error>>()?.join(", ")).unwrap();
        }
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            writeln!(out, "        let decoded = {};\n        family.validate(Self::PREPARED_DESCRIPTOR, value)?;\n        Ok(Self(decoded))", decode(representation, "value.nominal_representation().map_err(NativeBindingRefusal::InvalidValue)?", names)?).unwrap();
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            writeln!(
                out,
                "        let candidate = match value.variant_tag().map_err(NativeBindingRefusal::InvalidValue)? {{"
            )
            .unwrap();
            for case in cases {
                let variant = rust_pascal_identifier(case.tag())?;
                let boxed = options.boxed_variant_payloads.contains(&format!(
                    "{}.{}",
                    authored_name,
                    case.tag()
                ));
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
            writeln!(out, "            _ => Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)),\n        }}?;\n        family.validate(Self::PREPARED_DESCRIPTOR, value)?;\n        Ok(candidate)").unwrap();
        }
        _ => return Err(Error::InvalidSemanticType),
    }
    writeln!(out, "    }}\n}}\n").unwrap();
    let body_start = generated
        .find("        family.check_type")
        .ok_or(Error::InvalidSemanticType)?;
    let body_end = generated
        .rfind("    }\n}")
        .ok_or(Error::InvalidSemanticType)?;
    let scoped_body = generated[body_start..body_end]
        .replace("::from_borrowed_prepared(", "::from_borrowed_prepared_with_children(")
        .replace(", family)?", ", family, &__conduit_child_scope)?")
        .replace("        family.check_type(Self::PREPARED_DESCRIPTOR, value)?;", "        family.check_type(Self::PREPARED_DESCRIPTOR, value)?;\n        let __conduit_node_scope = scope.for_node(family, Self::PREPARED_DESCRIPTOR, value)?;\n        let __conduit_child_scope = __conduit_node_scope.child_scope()?;")
        .replace("family.validate(Self::PREPARED_DESCRIPTOR, value)?;", "if __conduit_node_scope.requires_validation() { family.validate(Self::PREPARED_DESCRIPTOR, value)?; }");
    let insertion = generated.rfind("\n}").ok_or(Error::InvalidSemanticType)?;
    destination.push_str(&generated[..insertion]);
    writeln!(destination, "\n    fn from_borrowed_prepared_with_children(value: conduit_core::ValidatedCanonicalStructuredValue<'_>, family: &mut conduit_plot::rust_binding::PreparedNativeFamily, scope: &conduit_plot::rust_binding::NativeChildAdmissionScope<'_>) -> Result<Self, NativeBindingRefusal> {{\n{scoped_body}    }}").unwrap();
    destination.push_str(&generated[insertion..]);
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
fn emit_external_shadow(
    out: &mut String,
    descriptor: &'static super::NativeFamilyTypeDescriptor,
    prefix: &str,
) -> Result<String, Error> {
    super::prepared_family_external::metadata_extent(descriptor)
        .map_err(|_| Error::InvalidSemanticType)?;
    let mut nodes = vec![descriptor];
    let mut cursor = 0;
    while cursor < nodes.len() {
        for child in nodes[cursor].children {
            if !nodes.iter().any(|node| core::ptr::eq(*node, *child)) {
                nodes.push(*child);
            }
        }
        cursor += 1;
    }
    for (index, node) in nodes.iter().enumerate() {
        let name = format!("{prefix}_{index}_EXPECTED");
        writeln!(out,"#[allow(non_upper_case_globals)]\nstatic {name}: conduit_plot::rust_binding::NativeFamilyTypeDescriptor = conduit_plot::rust_binding::NativeFamilyTypeDescriptor {{\n    type_bytes: {},\n    laws: &[{}],\n    contracts: &[", static_bytes(node.type_bytes), node.laws.iter().map(|law|static_bytes(law)).collect::<Vec<_>>().join(", ")).unwrap();
        for contract in node.contracts {
            writeln!(out,"        conduit_plot::rust_binding::NativeFamilyContractDescriptor {{ representation_path: {:?}, value_kind: {:?}, maximum_bytes: {}, constraints: &[",contract.representation_path,contract.value_kind,contract.maximum_bytes).unwrap();
            for constraint in contract.constraints {
                use super::NativeFamilyConstraintDescriptor as C;
                let recipe = match constraint {
                    C::CanonicalMembership { members, negated } => format!("CanonicalMembership {{ members: &[{}], negated: {negated} }}",members.iter().map(|member|static_bytes(member)).collect::<Vec<_>>().join(", ")),
                    C::FixedIntegerRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!("FixedIntegerRange {{ minimum: {}, maximum: {}, minimum_endpoint: conduit_core::IntervalEndpoint::{minimum_endpoint:?}, maximum_endpoint: conduit_core::IntervalEndpoint::{maximum_endpoint:?} }}",minimum.map_or_else(||"None".into(),|bytes|format!("Some({})",static_bytes(bytes))),maximum.map_or_else(||"None".into(),|bytes|format!("Some({})",static_bytes(bytes)))),
                };
                writeln!(out,"            conduit_plot::rust_binding::NativeFamilyConstraintDescriptor::{recipe},").unwrap();
            }
            writeln!(out, "        ] }},").unwrap();
        }
        let children = node
            .children
            .iter()
            .map(|child| {
                let index = nodes
                    .iter()
                    .position(|node| core::ptr::eq(*node, *child))
                    .expect("bounded complete graph collected");
                format!("&{prefix}_{index}_EXPECTED")
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(out,"    ],\n    children: &[{children}],\n    external_edges: &[],\n    conversion_profile: conduit_plot::rust_binding::NativeFamilyConversionProfile::{:?},\n    maximum_inline_bytes: 0,\n}};",node.conversion_profile).unwrap();
    }
    Ok(format!("{prefix}_0_EXPECTED"))
}

fn static_bytes(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(4).saturating_add(3));
    encoded.push_str("b\"");
    for byte in bytes {
        write!(encoded, "\\x{byte:02x}").unwrap();
    }
    encoded.push('"');
    encoded
}
