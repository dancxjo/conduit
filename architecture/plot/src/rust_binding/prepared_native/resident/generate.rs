//! Host-side specialization of the existing private native-admission templates.
use super::*;
use crate::rust_binding::resident_contract::{emit, EmissionRefusal};
use alloc::{collections::BTreeMap, format};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidentNativeGenerationRefusal {
    Admission(PreparedNativeRefusal),
    UnsupportedInvariant { scope: usize },
    UnsupportedContract { scope: usize, contract: usize },
    Bounds,
}
/// Emit only from the syntax checker's retained authoritative native catalogue.
/// Public inspection metadata cannot supply or replace missing owners/laws.
/// This finite profile retains fixed-integer predicates and refuses invariants
/// and other contract profiles. It publishes no offer or admission witness.
pub fn generate_resident_native_validator(
    document: &CheckedSyntaxDocument,
    identity: &KindId,
    bounds: PreparedNativeBounds,
    maximum_occurrences: usize,
) -> Result<String, ResidentNativeGenerationRefusal> {
    use ResidentNativeGenerationRefusal as R;
    let prepared = PreparedNativeAdmission::new_with_occurrence_limit(
        document,
        identity,
        bounds,
        maximum_occurrences,
    )
    .map_err(R::Admission)?;
    for (scope, value) in prepared.scopes.iter().enumerate() {
        if !value.native.invariants.is_empty() {
            return Err(R::UnsupportedInvariant { scope });
        }
    }
    let schema = prepared
        .native
        .value_type
        .canonical_bytes()
        .map_err(|_| R::Admission(PreparedNativeRefusal::InvalidProgram))?;
    let runtime = "conduit_plot::rust_binding";
    let mut source = format!(
        "pub const NATIVE_ID: &str = {:?};\nconst TYPE: &[u8] = &{schema:?};\n",
        identity.as_str()
    );
    let mut predicates = BTreeMap::new();
    let mut entries = Vec::new();
    for entry in &prepared.contracts {
        let encoded = entry.source.contract.identity_bytes();
        let next = predicates.len();
        let index = match predicates.entry(encoded) {
            alloc::collections::btree_map::Entry::Occupied(value) => *value.get(),
            alloc::collections::btree_map::Entry::Vacant(value) => {
                emit(
                    &mut source,
                    &entry.source.contract,
                    &format!("PREDICATE_{next}"),
                )
                .map_err(|reason| match reason {
                    EmissionRefusal::Invalid => R::Admission(PreparedNativeRefusal::InvalidProgram),
                    EmissionRefusal::Unsupported => R::UnsupportedContract {
                        scope: entry.scope,
                        contract: entry.contract,
                    },
                })?;
                value.insert(next);
                next
            }
        };
        let path = render_path(&entry.path);
        let scope = u16::try_from(entry.scope).map_err(|_| R::Bounds)?;
        let contract = u16::try_from(entry.contract).map_err(|_| R::Bounds)?;
        entries.push(format!("{runtime}::StaticNativeContract {{ scope: {scope}, contract: {contract}, path: &[{}], predicate: &PREDICATE_{index} }}", path));
    }
    let scopes: Vec<_> = prepared
        .scopes
        .iter()
        .map(|scope| format!("&[{}]", render_path(&scope.path)))
        .collect();
    source.push_str(&format!(
        "static SCOPES: [&[{runtime}::StaticNativePathStep]; {}] = [{}];\n",
        scopes.len(),
        scopes.join(",")
    ));
    let count = entries.len();
    source.push_str(&format!(
        "pub const CONTRACT_COUNT: usize = {count};\nstatic CONTRACTS: [{runtime}::StaticNativeContract; {count}] = [{}];\n",
        entries.join(",\n")
    ));
    source.push_str(&format!("pub const RUNTIME_STATE_BYTES: usize = core::mem::size_of::<{runtime}::ResidentNativeValidator<{count}>>();\npub fn prepare() -> Result<{runtime}::ResidentNativeValidator<{count}>, {runtime}::PreparedNativeRefusal> {{ {runtime}::ResidentNativeValidator::new(TYPE, {}, &CONTRACTS, &SCOPES, {maximum_occurrences}) }}\n", bounds.canonical_bytes));
    Ok(source)
}

fn render_path(path: &[Part]) -> String {
    path.iter()
        .map(|step| {
            let step = match step {
                Part::Representation => "Representation".into(),
                Part::Field(name) => format!("Field({name:?})"),
                Part::Each => "Each".into(),
                Part::Variant(name) => format!("Variant({name:?})"),
            };
            format!("conduit_plot::rust_binding::StaticNativePathStep::{step}")
        })
        .collect::<Vec<_>>()
        .join(",")
}
