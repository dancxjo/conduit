//! Compiled owner presentation of an already refused Native startup value.
use super::StartupCatalog;
use crate::{
    rust_binding::NativeBindingRefusal, ExpressionSyntax, LocalValue, Span, SyntaxCheckDiagnostic,
};
use alloc::{collections::BTreeMap, string::String};
use conduit_core::{StructuredInfoType, StructuredInfoValue};

/// Borrowed lexical custody for diagnostics, never a materialization/admission API.
#[derive(Clone, Copy)]
pub struct NativeRefinementSource<'a> {
    expression: &'a ExpressionSyntax,
    locals: &'a BTreeMap<String, &'a LocalValue>,
}
impl<'a> NativeRefinementSource<'a> {
    pub(crate) fn new(
        expression: &'a ExpressionSyntax,
        locals: &'a BTreeMap<String, &'a LocalValue>,
    ) -> Self {
        Self { expression, locals }
    }
    /// Locate a record field through the same bounded immutable Source locals.
    pub fn field_span(self, path: &str) -> Option<Span> {
        let mut expression = self.resolve(self.expression)?;
        for part in path.split('.').filter(|part| !part.is_empty()) {
            let ExpressionSyntax::Record { fields, .. } = expression else {
                return Some(expression.span());
            };
            expression =
                self.resolve(&fields.iter().find(|field| field.name.text == part)?.value)?;
        }
        Some(expression.span())
    }
    fn resolve(self, mut expression: &'a ExpressionSyntax) -> Option<&'a ExpressionSyntax> {
        for _ in 0..=self.locals.len() {
            let ExpressionSyntax::Atomic(token) = expression else {
                return Some(expression);
            };
            let Some(local) = self.locals.get(&token.text) else {
                return Some(expression);
            };
            expression = &local.value.syntax;
        }
        None
    }
}
pub type NativeRefinementDiagnosticCallback = for<'a> fn(
    &StructuredInfoValue,
    NativeRefinementSource<'a>,
    &NativeBindingRefusal,
) -> Option<SyntaxCheckDiagnostic>;

#[derive(Clone)]
pub(crate) struct NativeRefinementDiagnosticOwner {
    owner: String,
    value_type: StructuredInfoType,
    callback: NativeRefinementDiagnosticCallback,
}
impl core::fmt::Debug for NativeRefinementDiagnosticOwner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativeRefinementDiagnosticOwner")
            .field("owner", &self.owner)
            .field("value_type", &self.value_type)
            .finish_non_exhaustive()
    }
}
impl PartialEq for NativeRefinementDiagnosticOwner {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.value_type == other.value_type
            && core::ptr::fn_addr_eq(self.callback, other.callback)
    }
}
impl Eq for NativeRefinementDiagnosticOwner {}
impl StartupCatalog {
    /// Install a compiled domain's diagnostic mapping for one exact semantic
    /// Type. It is called only after Native laws refused the concrete value;
    /// returning None keeps that refusal. No result can admit the value.
    pub fn insert_native_refinement_diagnostic(
        &mut self,
        owner: impl Into<String>,
        value_type: StructuredInfoType,
        callback: NativeRefinementDiagnosticCallback,
    ) -> Result<(), String> {
        let owner = owner.into();
        if owner.is_empty() || owner.len() > 128 || value_type.profile().is_err() {
            return Err("invalid Native refinement diagnostic owner".into());
        }
        let entry = NativeRefinementDiagnosticOwner {
            owner,
            value_type,
            callback,
        };
        if let Some(existing) = self
            .native_refinement_diagnostics
            .iter()
            .find(|v| v.value_type == entry.value_type)
        {
            return if existing == &entry {
                Ok(())
            } else {
                Err("Native Type already has another diagnostic owner".into())
            };
        }
        if self.native_refinement_diagnostics.len() >= 32 {
            return Err("Native refinement diagnostic owner bound exceeded".into());
        }
        self.native_refinement_diagnostics.push(entry);
        Ok(())
    }
    pub(crate) fn native_refinement_diagnostic(
        &self,
        value: &StructuredInfoValue,
        source: NativeRefinementSource<'_>,
        refusal: &NativeBindingRefusal,
    ) -> Option<SyntaxCheckDiagnostic> {
        let owner = self
            .native_refinement_diagnostics
            .iter()
            .find(|v| &v.value_type == value.value_type())?;
        (owner.callback)(value, source, refusal)
    }
}
