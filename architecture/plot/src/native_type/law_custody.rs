//! Preparation-only native declaration custody. A claim is checked, never sealed by itself.
use crate::prelude::*;
use crate::{CheckedNativeType, StartupCatalog};
use alloc::collections::BTreeMap;
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Shared;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as Shared;
use conduit_core::StructuredInfoType;

pub(crate) mod family_material;
use family_material::{verify_family, NativeFamilyWitness};
pub use family_material::{
    NativeFamilyAnchor, NativeFamilyMaterial, NativeFamilyMember, NativeLawContext,
};
mod admission_metadata;
mod verification;
use verification::verify;
mod material_preflight;
mod root_source;

const MAXIMUM_MODULES: usize = 256;
const MAXIMUM_BYTES: usize = 4 * 1024 * 1024;
const MAXIMUM_DEPTH: usize = 16;

/// Untrusted decoded material. The external expected Type is deliberately absent:
/// verification must anchor the claim to the consumer's independently known Type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLawMaterial {
    pub source: String,
    pub declaration: String,
    pub imports: Vec<(String, NativeLawMaterial)>,
    pub families: Vec<(String, NativeFamilyMaterial)>,
    pub claimed: CheckedNativeType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeLawCustodyRefusal {
    Capacity,
    Source,
    UnsupportedContext,
    MissingContext,
    DuplicateImport,
    Declaration,
    Type,
    Laws,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Module {
    source: String,
    imports: Vec<(String, NativeLawWitness)>,
    families: Vec<(String, NativeFamilyWitness)>,
}

/// Opaque compiler-verified schema preimage and complete compiled law correspondence.
/// This is catalog preparation metadata, not a runtime value or an authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLawWitness {
    module: Shared<Module>,
    checked: CheckedNativeType,
}
impl NativeLawWitness {
    pub fn checked_type(&self) -> &CheckedNativeType {
        &self.checked
    }

    /// Bound the entire expanded export, including repeated shared dependencies,
    /// before cloning any Source, claim or child material.
    pub fn material(&self) -> Result<NativeLawMaterial, NativeLawCustodyRefusal> {
        admit_witness(self, 0, &mut Budget::default())?;
        Ok(expand_material(self))
    }

    /// Re-run native checking and existing schema derivation, including actual
    /// generic substitution. A new digest over supplied laws is not validation.
    pub fn verify(
        material: &NativeLawMaterial,
        expected: &StructuredInfoType,
    ) -> Result<Self, NativeLawCustodyRefusal> {
        Self::verify_with_context(material, expected, &NativeLawContext::default())
    }
    pub fn verify_with_context(
        material: &NativeLawMaterial,
        expected: &StructuredInfoType,
        context: &NativeLawContext<'_>,
    ) -> Result<Self, NativeLawCustodyRefusal> {
        material_preflight::check(material)?;
        context.validate()?;
        let mut budget = Budget::default();
        verify(material, expected, 0, &mut budget, context)
    }
    /// Obtain expected family bindings from the original opaque compiler witness,
    /// independently of any serialized/untrusted material supplied for replay.
    pub fn family_anchors(&self) -> Result<Vec<NativeFamilyAnchor>, NativeLawCustodyRefusal> {
        admit_witness(self, 0, &mut Budget::default())?;
        let mut values = BTreeMap::new();
        self.collect_anchors(&mut values)?;
        Ok(values.into_values().collect())
    }
    fn collect_anchors(
        &self,
        values: &mut BTreeMap<String, NativeFamilyAnchor>,
    ) -> Result<(), NativeLawCustodyRefusal> {
        for (_, witness) in &self.module.imports {
            witness.collect_anchors(values)?;
        }
        for (_, witness) in &self.module.families {
            witness.anchors(values)?;
        }
        Ok(())
    }

    pub(crate) fn preparation_bounds(&self) -> Result<(usize, usize), NativeLawCustodyRefusal> {
        let mut budget = Budget::default();
        admit_witness(self, 0, &mut budget)?;
        Ok((budget.modules, budget.bytes))
    }

    pub(crate) fn agrees(&self, checked: &CheckedNativeType) -> bool {
        self.checked.value_type == checked.value_type
            && self.checked.identity == checked.identity
            && self.checked.value_contracts == checked.value_contracts
            && self.checked.invariants == checked.invariants
    }
}

#[derive(Default, Clone)]
struct Budget {
    modules: usize,
    bytes: usize,
}
impl Budget {
    fn charge(&mut self, bytes: usize) -> Result<(), NativeLawCustodyRefusal> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(NativeLawCustodyRefusal::Capacity)?;
        if self.bytes > MAXIMUM_BYTES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        Ok(())
    }
    fn claims(&mut self, ty: &CheckedNativeType) -> Result<(), NativeLawCustodyRefusal> {
        if ty.invariants.len() > 4096 || ty.value_contracts.len() > 4096 {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        self.charge(ty.name.len())?;
        self.charge(ty.identity.as_str().len())?;
        self.charge(
            ty.value_type
                .canonical_bytes()
                .map_err(|_| NativeLawCustodyRefusal::Type)?
                .len(),
        )?;
        for contract in &ty.value_contracts {
            self.charge(contract.representation_path.len())?;
            self.contract(&contract.contract)?;
        }
        if ty.invariants.len() > 4096 || ty.value_contracts.len() > 4096 {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        for invariant in &ty.invariants {
            material_preflight::program(invariant, self)?;
            self.charge(
                invariant
                    .canonical_size()
                    .map_err(|_| NativeLawCustodyRefusal::Laws)?,
            )?;
        }
        Ok(())
    }
    fn contract(
        &mut self,
        contract: &conduit_core::CheckedValueContract,
    ) -> Result<(), NativeLawCustodyRefusal> {
        contract
            .validate_definition()
            .map_err(|_| NativeLawCustodyRefusal::Laws)?;
        let mut bytes = Some(0usize);
        contract.visit_identity_bytes(|chunk| {
            bytes = bytes.and_then(|n| n.checked_add(chunk.len()));
        });
        self.charge(bytes.ok_or(NativeLawCustodyRefusal::Capacity)?)
    }
    fn admit(&mut self, source: &str, depth: usize) -> Result<(), NativeLawCustodyRefusal> {
        self.modules = self
            .modules
            .checked_add(1)
            .ok_or(NativeLawCustodyRefusal::Capacity)?;
        self.bytes = self
            .bytes
            .checked_add(source.len())
            .ok_or(NativeLawCustodyRefusal::Capacity)?;
        if depth > MAXIMUM_DEPTH || self.modules > MAXIMUM_MODULES || self.bytes > MAXIMUM_BYTES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        Ok(())
    }
}
fn check_module(
    source: &str,
    base: &StartupCatalog,
) -> Result<Vec<CheckedNativeType>, NativeLawCustodyRefusal> {
    let syntax = crate::parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(NativeLawCustodyRefusal::Source);
    }
    // Physical/custom glyph owners are not reconstructed from native laws.
    if !syntax.dimensions.is_empty() || !syntax.units.is_empty() || !syntax.prefixes.is_empty() {
        return Err(NativeLawCustodyRefusal::UnsupportedContext);
    }
    super::generic::budget::validate_iter(syntax.types.iter())
        .map_err(|_| NativeLawCustodyRefusal::Capacity)?;
    let aliases = super::install_import_aliases(&syntax, base)
        .map_err(|_| NativeLawCustodyRefusal::UnsupportedContext)?;
    let (public, catalog) = super::check_native_types(&syntax.types, &aliases)
        .map_err(|_| NativeLawCustodyRefusal::Source)?;
    let mut retained = catalog.retained_native_types();
    for ty in public {
        if !retained.iter().any(|existing| existing.name == ty.name) {
            retained.push(ty);
        }
    }
    Ok(retained)
}

/// Capture only when replay with verified native dependencies reproduces every
/// claim. Legacy custom catalog entries never become silently unconstrained.
pub(crate) fn collect(
    syntax: &crate::SyntaxDocument,
    base: &StartupCatalog,
    declarations: &[CheckedNativeType],
) -> BTreeMap<String, NativeLawWitness> {
    let mut result = BTreeMap::new();
    if declarations.is_empty() || syntax.round_trip().len() > MAXIMUM_BYTES {
        return result;
    }
    let mut retained_budget = Budget::default();
    for checked in declarations {
        let Ok(source) = root_source::extract(syntax, &checked.name) else {
            continue;
        };
        let closed = crate::parse_syntax_document(&source);
        if let Some(witness) = collect_root(&closed, base, checked) {
            // The returned registry is bounded too, not just each root export.
            let mut candidate = retained_budget.clone();
            if admit_witness(&witness, 0, &mut candidate).is_err() {
                break;
            }
            retained_budget = candidate;
            result.insert(checked.name.clone(), witness);
        }
    }
    result
}

fn collect_root(
    syntax: &crate::SyntaxDocument,
    base: &StartupCatalog,
    checked: &CheckedNativeType,
) -> Option<NativeLawWitness> {
    let required = required_imports(syntax);
    if required.len() > MAXIMUM_MODULES {
        return None;
    }
    let mut borrowed = Vec::new();
    let mut borrowed_families = Vec::new();
    for name in required {
        if let Some(witness) = base.native_law_witness(&name) {
            borrowed.push((name, witness));
        } else {
            borrowed_families.push((
                name.clone(),
                base.native_families.get(&name)?.source_custody.as_ref()?,
            ));
        }
    }
    let mut budget = Budget::default();
    budget.admit(syntax.round_trip(), 0).ok()?;
    budget.charge(checked.name.len()).ok()?;
    budget.claims(checked).ok()?;
    for (name, witness) in &borrowed {
        budget.charge(name.len()).ok()?;
        admit_witness(witness, 1, &mut budget).ok()?;
    }
    for (name, witness) in &borrowed_families {
        budget.charge(name.len()).ok()?;
        witness.admit(1, &mut budget).ok()?;
    }
    let mut replay = StartupCatalog::new();
    for (name, witness) in &borrowed {
        replay
            .insert_checked_native_type(name.clone(), witness.checked_type())
            .ok()?;
    }
    for (name, witness) in &borrowed_families {
        witness.install(name, &mut replay).ok()?;
    }
    let rechecked = check_module(syntax.round_trip(), &replay).ok()?;
    if !rechecked.iter().any(|ty| ty == checked) {
        return None;
    }
    let imports = borrowed
        .into_iter()
        .map(|(name, witness)| (name, witness.clone()))
        .collect();
    Some(NativeLawWitness {
        module: Shared::new(Module {
            source: syntax.round_trip().into(),
            imports,
            families: borrowed_families
                .into_iter()
                .map(|(name, witness)| (name, witness.clone()))
                .collect(),
        }),
        checked: checked.clone(),
    })
}

fn admit_witness(
    witness: &NativeLawWitness,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), NativeLawCustodyRefusal> {
    budget.admit(&witness.module.source, depth)?;
    budget.charge(witness.checked.name.len())?;
    budget.claims(&witness.checked)?;
    for (name, dependency) in &witness.module.imports {
        budget.charge(name.len())?;
        admit_witness(dependency, depth + 1, budget)?;
    }
    for (name, witness) in &witness.module.families {
        budget.charge(name.len())?;
        witness.admit(depth + 1, budget)?;
    }
    Ok(())
}

fn required_imports(syntax: &crate::SyntaxDocument) -> Vec<String> {
    let builtins = StartupCatalog::new();
    let mut required = alloc::collections::BTreeSet::new();
    for declaration in &syntax.types {
        let mut names = Vec::new();
        super::definition_references(&declaration.definition, &mut names);
        for parameter in &declaration.parameters {
            if let Some(annotation) = &parameter.value_type {
                super::expression_references(annotation, &mut names);
            }
        }
        for name in names {
            if declaration.parameters.iter().any(|p| p.name.text == name)
                || syntax.types.iter().any(|ty| ty.name.text == name)
                || crate::value_type::checked_value_type(name, &builtins).is_ok()
            {
                continue;
            }
            let original = syntax
                .uses
                .iter()
                .find(|u| u.alias.text == name)
                .map_or(name, |u| u.path.as_str());
            required.insert(original.into());
        }
    }
    required.into_iter().collect()
}

// Only called after one complete immutable-witness preflight succeeds.
fn expand_material(witness: &NativeLawWitness) -> NativeLawMaterial {
    NativeLawMaterial {
        source: witness.module.source.clone(),
        declaration: witness.checked.name.clone(),
        imports: witness
            .module
            .imports
            .iter()
            .map(|(name, child)| (name.clone(), expand_material(child)))
            .collect(),
        families: witness
            .module
            .families
            .iter()
            .map(|(name, witness)| (name.clone(), witness.material()))
            .collect(),
        claimed: witness.checked.clone(),
    }
}

#[cfg(test)]
#[path = "law_custody/export_bounds_tests.rs"]
mod export_bounds_tests;

pub(crate) fn collect_documents(
    documents: &[crate::SyntaxDocument],
    base: &StartupCatalog,
    checked: &[CheckedNativeType],
) -> BTreeMap<String, NativeLawWitness> {
    let Ok(source) = root_source::merge_documents(documents) else {
        return BTreeMap::new();
    };
    let syntax = crate::parse_syntax_document(&source);
    if !syntax.diagnostics.is_empty() {
        return BTreeMap::new();
    }
    collect(&syntax, base, checked)
}
