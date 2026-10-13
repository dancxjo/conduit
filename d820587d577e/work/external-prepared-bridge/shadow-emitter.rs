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
            writeln!(out,"        ] }},").unwrap();
        }
        let children = node.children.iter().map(|child|{
            let index=nodes.iter().position(|node|core::ptr::eq(*node,*child)).expect("bounded complete graph collected");
            format!("&{prefix}_{index}_EXPECTED")
        }).collect::<Vec<_>>().join(", ");
        writeln!(out,"    ],\n    children: &[{children}],\n    external_edges: &[],\n    conversion_profile: conduit_plot::rust_binding::NativeFamilyConversionProfile::{:?},\n    maximum_inline_bytes: 0,\n}};",node.conversion_profile).unwrap();
    }
    Ok(format!("{prefix}_0_EXPECTED"))
}

fn static_bytes(bytes: &[u8]) -> String {
    let mut encoded=String::with_capacity(bytes.len().saturating_mul(4).saturating_add(3));
    encoded.push_str("b\"");
    for byte in bytes { write!(encoded,"\\x{byte:02x}").unwrap(); }
    encoded.push('"');
    encoded
}
