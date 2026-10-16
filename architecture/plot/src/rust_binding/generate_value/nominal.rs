//! Emit the checked nominal Rust binding and its canonical value conversions.
use super::*;

pub(super) fn emit_nominal(
    out: &mut String,
    rust_name: &str,
    constant: &str,
    representation: &StructuredInfoType,
    names: &BTreeMap<String, String>,
    contracts: &[NativeTypeValueContract],
    copy: bool,
) -> Result<(), RustBindingGenerationError> {
    let inner = rust_type(representation, names)?;
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    writeln!(
        out,
        "    pub fn new(value: {inner}) -> Result<Self, NativeBindingRefusal> {{"
    )
    .expect("String writing is infallible");
    writeln!(out, "        let candidate = Self(value);").expect("String writing is infallible");
    let directly_checked_integer = !contracts.is_empty()
        && contracts.iter().all(|contract| {
            contract.representation_path.is_empty()
                && !contract.contract.constraints.is_empty()
                && contract.contract.constraints.iter().all(|constraint| {
                    matches!(constraint, ValueConstraint::FixedIntegerRange { .. })
                })
        });
    let direct_kind = match representation.shape() {
        StructuredInfoTypeShape::Leaf(kind) if directly_checked_integer => Some(kind),
        _ => None,
    };
    if let Some(kind) = direct_kind {
        let primitive = primitive_rust_type(kind.as_str())?;
        emit_direct_integer_checks(out, "value", &primitive, contracts)?;
    } else {
        writeln!(
            out,
            "        let structured = candidate{}.into_structured()?;",
            if copy { "" } else { ".clone()" }
        )
        .expect("String writing is infallible");
        writeln!(out, "        conduit_plot::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?;")
            .expect("String writing is infallible");
    }
    writeln!(out, "        Ok(candidate)\n    }}\n}}\n").expect("String writing is infallible");

    writeln!(out, "impl NativeRustBinding for {rust_name} {{")
        .expect("String writing is infallible");
    writeln!(out, "    fn semantic_type() -> Result<StructuredInfoType, NativeBindingRefusal> {{ Self::semantic_type() }}")
        .expect("String writing is infallible");
    writeln!(
        out,
        "    fn into_structured(self) -> Result<StructuredInfoValue, NativeBindingRefusal> {{"
    )
    .expect("String writing is infallible");
    writeln!(out, "        let semantic = StructuredInfoType::from_canonical_bytes({constant}).map_err(NativeBindingRefusal::InvalidSemanticType)?;")
        .expect("String writing is infallible");
    writeln!(out, "        let representation = conduit_plot::rust_binding::nominal_representation_type(&semantic)?;")
        .expect("String writing is infallible");
    let encoded = super::super::generate_conversion::encode_expression(
        representation,
        "self.0",
        "representation.clone()",
        names,
    )?;
    writeln!(out, "        let value = {encoded};").expect("String writing is infallible");
    writeln!(out, "        StructuredInfoValue::nominal(semantic, value).map_err(NativeBindingRefusal::InvalidValue)\n    }}")
        .expect("String writing is infallible");
    writeln!(out, "    fn from_structured(value: StructuredInfoValue) -> Result<Self, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    writeln!(out, "        if value.value_type() != &Self::semantic_type()? {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}")
        .expect("String writing is infallible");
    let decoded =
        super::super::generate_conversion::decode_expression(representation, "value", names)?;
    if direct_kind.is_some() {
        // Integer constructors already check their exact bounds directly. Keep
        // that path rather than materializing generic refinement metadata.
        writeln!(out, "        Self::new({decoded})\n    }}\n}}\n")
            .expect("String writing is infallible");
    } else {
        // The incoming value already has the exact nominal Type. Validate its
        // retained refinements without rebuilding a second structured tree.
        writeln!(out, "        conduit_plot::rust_binding::validate_native_contracts(&value, &Self::value_contracts())?;")
            .expect("String writing is infallible");
        writeln!(out, "        Ok(Self({decoded}))\n    }}\n}}\n")
            .expect("String writing is infallible");
    }
    Ok(())
}
