//! Installed checking-time Kind, Type and authoring contracts.
mod type_catalog;
mod typed_literal;
use super::{CheckedNativeType, KindSignature, NativeTypeSourceOrigin, NativeTypeValueContract};
use crate::prelude::*;
use alloc::collections::BTreeMap;
use conduit_core::CheckedFront;
pub use typed_literal::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupCatalog {
    pub(crate) physical: crate::physical_declarations::CheckedPhysicalCatalogue,
    pub(crate) physical_sources: Vec<crate::SyntaxDocument>,
    pub(crate) literal_owners:
        BTreeMap<String, crate::glyph_notation::compiled::InstalledLiteralOwner>,
    kinds: BTreeMap<String, KindSignature>,
    pub(crate) prepared_source_values:
        BTreeMap<(usize, usize), (crate::Expression, crate::CanonicalStructuredStartupValue)>,
    // Populated only by document-scoped receipt admission; never an ambient alias.
    pub(crate) prepared_glyph_values: BTreeMap<
        (usize, usize),
        (
            crate::TypedGlyphLiteralSyntax,
            crate::CanonicalStructuredStartupValue,
        ),
    >,
    typed_literal_families: BTreeMap<String, TypedLiteralFamily>,
    fores: BTreeMap<String, CheckedFront>,
    variadic_fores: BTreeMap<String, crate::HomogeneousVariadicFore>,
    structured_types: BTreeMap<String, conduit_core::StructuredInfoType>,
    structured_type_contracts: BTreeMap<String, Vec<NativeTypeValueContract>>,
    structured_type_invariants: BTreeMap<String, Vec<crate::PortableExpressionProgram>>,
    value_kind_aliases: BTreeMap<String, conduit_core::KindId>,
    pub(crate) native_type_sources: BTreeMap<String, NativeTypeSourceOrigin>,
    pub(crate) native_families: BTreeMap<String, crate::native_type::family::NativeTypeFamily>,
    pub(crate) exact_initial_info: BTreeMap<(conduit_core::KindId, String), Vec<u8>>,
}

impl StartupCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, signature: KindSignature) -> Result<(), String> {
        if self.kinds.contains_key(&signature.kind)
            || self.typed_literal_families.contains_key(&signature.kind)
        {
            return Err(format!(
                "duplicate startup signature for kind '{}'",
                signature.kind
            ));
        }
        let mut names = BTreeMap::new();
        for parameter in &signature.startup_parameters {
            if names.insert(parameter.name.as_str(), ()).is_some() {
                return Err(format!(
                    "duplicate startup parameter '{}' for kind '{}'",
                    parameter.name, signature.kind
                ));
            }
        }
        self.kinds.insert(signature.kind.clone(), signature);
        Ok(())
    }

    pub(crate) fn get(&self, kind: &str) -> Option<&KindSignature> {
        self.kinds.get(kind)
    }

    /// Returns the exact startup Front installed for one semantic Kind.
    ///
    /// Reusable Back installers use this together with the profile contract so
    /// substitution checks the complete Front, including startup parameters.
    pub fn signature(&self, kind: &str) -> Option<&KindSignature> {
        self.kinds.get(kind)
    }

    /// Installs the complete checked Fore for source-time laws which depend on
    /// runtime port shape, such as glyph operand binding. This is not a Back or
    /// planning profile: it carries no implementation or availability truth.
    pub fn insert_fore(&mut self, kind: &str, fore: CheckedFront) -> Result<(), String> {
        let signature = self
            .kinds
            .get(kind)
            .ok_or_else(|| format!("cannot install a Fore for unknown Kind '{kind}'"))?;
        let expected = self
            .canonical_startup_parameters(signature)
            .map_err(|error| format!("invalid startup Fore for Kind '{kind}': {error:?}"))?;
        if fore.startup_parameters() != expected {
            return Err(format!(
                "checked Fore startup parameters differ from Kind '{kind}' signature"
            ));
        }
        if self.fores.contains_key(kind) {
            return Err(format!("duplicate checked Fore for Kind '{kind}'"));
        }
        self.fores.insert(kind.to_string(), fore);
        Ok(())
    }

    pub fn fore(&self, kind: &str) -> Option<&CheckedFront> {
        self.fores.get(kind)
    }

    /// Marks one reviewed Kind Fore as a finite homogeneous input family.
    /// Each use is specialized to ordinary exact ports before checking ends.
    pub fn insert_homogeneous_variadic_fore(
        &mut self,
        kind: &str,
        family: crate::HomogeneousVariadicFore,
    ) -> Result<(), String> {
        let signature = self
            .kinds
            .get(kind)
            .ok_or_else(|| format!("cannot install a variadic Fore for unknown Kind '{kind}'"))?;
        let startup_parameters = self
            .canonical_startup_parameters(signature)
            .map_err(|error| format!("invalid variadic Fore for Kind '{kind}': {error:?}"))?;
        family.specialize(usize::from(family.minimum_inputs()), startup_parameters)?;
        if self.variadic_fores.contains_key(kind) {
            return Err(format!("duplicate variadic Fore for Kind '{kind}'"));
        }
        self.variadic_fores.insert(kind.into(), family);
        Ok(())
    }

    pub(crate) fn fore_for_arity(
        &self,
        kind: &str,
        input_count: usize,
    ) -> Result<Option<CheckedFront>, String> {
        if let Some(family) = self.variadic_fores.get(kind) {
            let signature = self
                .kinds
                .get(kind)
                .expect("variadic Fores retain their Kind signature");
            let startup_parameters = self
                .canonical_startup_parameters(signature)
                .map_err(|error| format!("invalid variadic Fore for Kind '{kind}': {error:?}"))?;
            return family.specialize(input_count, startup_parameters).map(Some);
        }
        Ok(self
            .fores
            .get(kind)
            .filter(|fore| fore.inputs().len() == input_count)
            .cloned())
    }

    /// Resolves authoring spellings into the canonical startup type identities
    /// carried by checked fronts and realization offers.
    pub fn canonical_startup_parameters(
        &self,
        signature: &KindSignature,
    ) -> Result<Vec<conduit_core::FrontStartupParameter>, conduit_core::StructuredInfoRefusal> {
        signature
            .startup_parameters
            .iter()
            .map(|parameter| {
                Ok(conduit_core::FrontStartupParameter {
                    name: parameter.name.clone(),
                    value_type: crate::value_type::checked_value_kind(&parameter.value_type, self)?,
                    has_default: parameter.default.is_some(),
                })
            })
            .collect()
    }
}
