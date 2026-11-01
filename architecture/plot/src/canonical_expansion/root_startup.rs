//! Independently prepared, finite Root invocation values. Never inferred from an expansion.
use super::*;
use crate::CheckedStartupBinding;
use conduit_core::{
    FrontValueLocation, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};

pub const MAX_ROOT_STARTUP_INPUTS: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootStartupRefusal {
    MissingRoot,
    Capacity,
    UnknownInput,
    DuplicateInput,
    WrongType,
    NonConcrete,
    ValueContract,
}

/// Sealed preparation custody includes the checked declarations and their laws.
/// Preparation may allocate; neither expansion nor a Plan can mint this evidence.
#[derive(Clone)]
pub struct PreparedRootStartupInputs {
    document: CheckedSyntaxDocument,
    root: String,
    values: BTreeMap<String, CanonicalStartupValue>,
}
impl core::fmt::Debug for PreparedRootStartupInputs {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PreparedRootStartupInputs")
            .field("root", &self.root)
            .field("source_document_id", &self.document.source_document_id)
            .field("input_count", &self.values.len())
            .finish_non_exhaustive()
    }
}
impl PreparedRootStartupInputs {
    pub fn prepare(
        document: &CheckedSyntaxDocument,
        root: &str,
        catalog: &ProfileCatalog,
        inputs: &[CheckedStartupBinding],
    ) -> Result<Self, RootStartupRefusal> {
        use RootStartupRefusal as R;
        let plot = document
            .plots
            .iter()
            .find(|p| p.name == root)
            .ok_or(R::MissingRoot)?;
        if crate::syntax_check::is_local_plot_identity(root) {
            return Err(R::MissingRoot);
        }
        if inputs.len() > MAX_ROOT_STARTUP_INPUTS {
            return Err(R::Capacity);
        }
        let startup = catalog.startup_catalog().map_err(|_| R::WrongType)?;
        let front = plot.checked_front();
        let mut values = BTreeMap::new();
        let mut total = 0usize;
        for input in inputs {
            if values.contains_key(&input.name) {
                return Err(R::DuplicateInput);
            }
            let parameter = plot
                .startup_parameters
                .iter()
                .find(|p| p.name == input.name)
                .ok_or(R::UnknownInput)?;
            if parameter.value_type != input.value_type {
                return Err(R::WrongType);
            }
            let kind = &front
                .startup_parameters()
                .iter()
                .find(|p| p.name == input.name)
                .ok_or(R::WrongType)?
                .value_type;
            let expected = document
                .structured_type(kind)
                .cloned()
                .or_else(|| startup.structured_type(&parameter.value_type).cloned())
                .or_else(|| StructuredInfoType::leaf(kind.clone()).ok())
                .ok_or(R::WrongType)?;
            let value = concrete(&input.value, &expected, document)?;
            let encoded = value.canonical_bytes().map_err(|_| R::Capacity)?;
            total = total.checked_add(encoded.len()).ok_or(R::Capacity)?;
            if total > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
                return Err(R::Capacity);
            }
            let payload = match (expected.shape(), value.shape()) {
                (StructuredInfoTypeShape::Leaf(_), StructuredInfoValueShape::Leaf(bytes)) => bytes,
                _ => encoded.as_slice(),
            };
            if parameter
                .maximum_bytes
                .is_some_and(|max| payload.len() as u64 > max)
            {
                return Err(R::ValueContract);
            }
            for contract in front
                .value_contracts()
                .iter()
                .filter(|c| c.location == FrontValueLocation::Startup(input.name.clone()))
            {
                contract
                    .contract
                    .validate(payload)
                    .map_err(|_| R::ValueContract)?;
            }
            for native in document
                .native_types
                .iter()
                .chain(&document.retained_native_types)
                .filter(|native| native.value_type == expected)
            {
                crate::rust_binding::validate_native_contracts(&value, &native.value_contracts)
                    .map_err(|_| R::ValueContract)?;
                crate::rust_binding::validate_native_invariants(&value, &native.invariants)
                    .map_err(|_| R::ValueContract)?;
            }
            values.insert(input.name.clone(), input.value.clone());
        }
        Ok(Self {
            document: document.clone(),
            root: root.into(),
            values,
        })
    }
    pub fn root_name(&self) -> &str {
        &self.root
    }
    pub(super) fn value(&self, name: &str) -> Option<&CanonicalStartupValue> {
        self.values.get(name)
    }
    pub(super) fn require_source(
        &self,
        document: &CheckedSyntaxDocument,
        root: &str,
    ) -> Result<(), CanonicalExpansionDiagnostic> {
        if self.root != root || self.document != *document {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-065",
                "prepared Root inputs belong to another checked declaration".into(),
            ));
        }
        Ok(())
    }
}
fn concrete(
    value: &CanonicalStartupValue,
    expected: &StructuredInfoType,
    document: &CheckedSyntaxDocument,
) -> Result<StructuredInfoValue, RootStartupRefusal> {
    use RootStartupRefusal as R;
    if let CanonicalStartupValue::Structured(value) = value {
        if value.value_type() != expected {
            return Err(R::WrongType);
        }
        return value.try_concrete().ok_or(R::NonConcrete);
    }
    let StructuredInfoTypeShape::Leaf(kind) = expected.shape() else {
        return Err(R::WrongType);
    };
    let bytes = match value {
        CanonicalStartupValue::Literal(literal) => {
            if literal.len() > conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES {
                return Err(R::Capacity);
            }
            if conduit_core::primitive_info_kind(kind.as_str()).is_none()
                && kind.as_str() != conduit_core::MATHEMATICAL_VALUE_ID
                && conduit_core::parse_quantity_role_info_id(kind.as_str()).is_none()
            {
                let bytes = document
                    .exact_initial_info
                    .get(&(kind.clone(), literal.clone()))
                    .ok_or(R::WrongType)?
                    .clone();
                return StructuredInfoValue::leaf(expected.clone(), bytes)
                    .map_err(|_| R::WrongType);
            }
            let span = crate::Span {
                start: 0,
                end: 0,
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
            };
            crate::structured_startup::canonical_leaf_literal(kind.as_str(), literal, span)
                .map_err(|_| R::WrongType)?
        }
        CanonicalStartupValue::Quantity(value)
            if kind.as_str() == conduit_core::QUANTITY_INFO_ID
                || conduit_core::quantity_info_dimension(kind.as_str()).is_some()
                || conduit_core::parse_quantity_role_info_id(kind.as_str()).is_some() =>
        {
            value.value().encode().to_vec()
        }
        CanonicalStartupValue::Unit(value) if kind.as_str() == conduit_core::UNIT_INFO_ID => {
            value.value().encode().to_vec()
        }
        CanonicalStartupValue::TemperatureDifference(value)
            if kind.as_str() == conduit_core::EXACT_TEMPERATURE_DIFFERENCE_INFO_ID =>
        {
            crate::quantity_conversion::profile_source_difference(value.value())
                .map_err(|_| R::WrongType)?
                .canonical_bytes()
                .map_err(|_| R::Capacity)?
        }
        CanonicalStartupValue::Quantity(_)
        | CanonicalStartupValue::Unit(_)
        | CanonicalStartupValue::TemperatureDifference(_) => return Err(R::WrongType),
        _ => return Err(R::NonConcrete),
    };
    StructuredInfoValue::leaf(expected.clone(), bytes).map_err(|_| R::WrongType)
}
