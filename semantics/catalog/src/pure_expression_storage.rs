//! Initial descriptor funding for one qualified ordinary Some(ResourceRef).
//! This is not a per-Play reconstruction path. The initialized Host retains the
//! exact Source-checked offer witness; decoding/program custody is paid separately.
use conduit_core as c;
use conduit_plot::{
    PortableExpressionProgram, QualifiedExpressionProgramStorage, QualifiedExpressionStorage,
    QualifiedExpressionStorageRefusal as E,
};
use core::mem::size_of;
const OPTIONAL: &str = "conduit.conduitese.optional.v1";
const PROFILE: usize = "structured-info/profile-".len() + 64 + "@1".len();
const ID: usize = "conduitese/pure-expression/".len() + 64;
const REVISION: &str = conduit_plot::PURE_EXPRESSION_REVISION;
fn prefix() -> usize {
    9 + OPTIONAL.len()
        + 8
        + 5
        + c::EMPTY_INFO_ID.len()
        + 8
        + 5
        + c::RESOURCE_REFERENCE_INFO_ID.len()
}
fn program_bytes(bounded: bool) -> usize {
    "conduit.pure-expression.program.v2".len()
        + usize::from(bounded) * 4
        + 4 * 8
        + 2 * (5 + c::RESOURCE_REFERENCE_INFO_ID.len())
        + 2 * prefix()
        + 14
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualifiedSomeExpressionContractStorage {
    /// Peak requested material including inline resulting Kind. Excludes the
    /// caller-owned decoded program/configuration and ordinary call stack.
    pub preparation_bytes: usize,
    /// Inline Kind plus every reachable final allocation.
    pub retained_bytes: usize,
}
impl QualifiedSomeExpressionContractStorage {
    pub fn for_program(program: &PortableExpressionProgram) -> Result<Self, E> {
        QualifiedExpressionStorage::for_resource_reference_some(program)?;
        Ok(Self::model(program.input_maximum_bytes.is_some()))
    }
    pub fn for_canonical_hex(encoded: &str) -> Result<Self, E> {
        QualifiedExpressionProgramStorage::resource_reference_some_hex(encoded)?;
        Ok(Self::model(encoded.len() / 2 == program_bytes(true)))
    }
    fn model(bounded: bool) -> Self {
        let reference = c::RESOURCE_REFERENCE_INFO_ID.len();
        let laws = if bounded { 9 } else { 8 };
        // One startup parameter; shorthand pair; exact Kind identity/revision;
        // two finite ports; one program Text configuration; pure semantic laws.
        let mut retained_bytes = size_of::<c::Kind>()
            + size_of::<c::FrontStartupParameter>()
            + "program".len()
            + "value/text".len()
            + "input".len()
            + "output".len()
            + ID
            + REVISION.len()
            + 2 * size_of::<c::PortDescriptor>()
            + "input".len()
            + reference
            + "output".len()
            + PROFILE
            + size_of::<c::KindConfigurationField>()
            + "program".len()
            + 2 * program_bytes(bounded)
            + laws * size_of::<c::KindSemanticLaw>();
        if bounded {
            retained_bytes += size_of::<c::FrontValueContract>() + "input".len() + reference;
        }
        // Core profile construction borrows/clones the exact optional Type,
        // serializes its prefix and creates its 64-byte digest and profile ID.
        // Shared schema aliases are charged in full, as in Core's backing model.
        let type_clone = c::StructuredInfoType::compound_backing_bytes()
            + OPTIONAL.len()
            + 2 * size_of::<c::StructuredVariantCase>()
            + 8
            + c::EMPTY_INFO_ID.len()
            + reference;
        // Expression identity digest; two temporary input-Kind clones in
        // semantic_laws_for_identity; v3 reserve_exact replaces the old 8-law
        // table while the final 9-law table and owned members remain live.
        let preparation_bytes = retained_bytes
            + type_clone
            + prefix()
            + 64
            + PROFILE
            + 64
            + 2 * reference
            + if bounded {
                8 * size_of::<c::KindSemanticLaw>()
            } else {
                0
            };
        Self {
            preparation_bytes,
            retained_bytes,
        }
    }
}

pub fn prepare_qualified_resource_reference_some_contract(
    program: &PortableExpressionProgram,
    temporal: c::PortTemporal,
    admitted_bytes: usize,
) -> Result<c::Kind, E> {
    let receipt = QualifiedSomeExpressionContractStorage::for_program(program)?;
    if !matches!(
        temporal,
        c::PortTemporal::Value | c::PortTemporal::Flow { closes: true }
    ) {
        return Err(E::UnsupportedProgram);
    }
    if admitted_bytes < receipt.preparation_bytes {
        return Err(E::InsufficientPreparation);
    }
    let kind = super::pure_expression::pure_expression_contract(program, temporal)
        .map_err(|_| E::InvalidPreparedStorage)?;
    if size_of::<c::Kind>() + qualified_resource_reference_some_contract_retained_bytes(&kind)?
        > receipt.retained_bytes
    {
        return Err(E::InvalidPreparedStorage);
    }
    Ok(kind)
}
/// Actual reachable capacities excluding inline Kind. Unsupported owned law,
/// configuration and port shapes refuse instead of counting only their headers.
pub fn qualified_resource_reference_some_contract_retained_bytes(
    kind: &c::Kind,
) -> Result<usize, E> {
    let mut bytes = Bytes(Some(0));
    bytes.add(kind.kind_id.0.capacity());
    bytes.add(kind.kind_contract_revision.0.capacity());
    bytes.table::<c::FrontStartupParameter>(kind.startup_parameters.capacity());
    bytes.table::<c::PortDescriptor>(kind.inputs.capacity());
    bytes.table::<c::PortDescriptor>(kind.outputs.capacity());
    bytes.table::<c::KindConfigurationField>(kind.configuration.capacity());
    bytes.table::<c::KindSemanticLaw>(kind.semantic_laws.capacity());
    if let Some((input, output)) = &kind.shorthand {
        bytes.add(input.0.capacity());
        bytes.add(output.0.capacity());
    }
    for parameter in &kind.startup_parameters {
        bytes.add(parameter.name.capacity());
        bytes.add(parameter.value_type.0.capacity());
    }
    for port in kind.inputs.iter().chain(&kind.outputs) {
        if port.abnormal_kind.is_some() {
            return Err(E::UnsupportedProgram);
        }
        bytes.add(port.port_id.0.capacity());
        bytes.add(port.value_kind.0.capacity());
    }
    for field in &kind.configuration {
        let c::ConfigurationValue::Text(text) = &field.default_value else {
            return Err(E::UnsupportedProgram);
        };
        if !matches!(field.rule, c::KindConfigurationRule::TextBytes { .. }) {
            return Err(E::UnsupportedProgram);
        }
        bytes.add(field.key.capacity());
        bytes.add(text.capacity());
    }
    for law in &kind.semantic_laws {
        match law {
            c::KindSemanticLaw::ValueContracts(contracts) => {
                bytes.table::<c::FrontValueContract>(contracts.capacity());
                for contract in contracts {
                    if !contract.contract.constraints.is_empty() {
                        return Err(E::UnsupportedProgram);
                    }
                    // An empty Vec may still retain capacity after mutation.
                    bytes.table::<c::ValueConstraint>(contract.contract.constraints.capacity());
                    bytes.add(contract.contract.value_kind.0.capacity());
                    match &contract.location {
                        c::FrontValueLocation::Input(port)
                        | c::FrontValueLocation::Output(port) => bytes.add(port.0.capacity()),
                        _ => return Err(E::UnsupportedProgram),
                    }
                }
            }
            c::KindSemanticLaw::ExternalEffects(_)
            | c::KindSemanticLaw::TemporalState(_)
            | c::KindSemanticLaw::TimeDependence(_)
            | c::KindSemanticLaw::RandomDependence(_)
            | c::KindSemanticLaw::ResourceDependence(_)
            | c::KindSemanticLaw::Suspension(_)
            | c::KindSemanticLaw::Variability(_)
            | c::KindSemanticLaw::Replay(_) => (),
            _ => return Err(E::UnsupportedProgram),
        }
    }
    bytes.0.ok_or(E::InvalidPreparedStorage)
}
struct Bytes(Option<usize>);
impl Bytes {
    fn add(&mut self, bytes: usize) {
        self.0 = self.0.and_then(|n| n.checked_add(bytes));
    }
    fn table<T>(&mut self, capacity: usize) {
        self.0 = self
            .0
            .and_then(|n| n.checked_add(capacity.checked_mul(size_of::<T>())?));
    }
}
