//! Preparation cache identity is separate from the established nominal contract.
use super::{binding::Bindings, canonical, Context};
use crate::prelude::*;
use crate::{NativeTypeArgumentSyntax as Argument, TypeExpressionSyntax as Type, TypeSyntax};
use alloc::collections::BTreeMap;

impl Context<'_> {
    pub(super) fn semantic_key(
        &self,
        origin: &TypeSyntax,
        arguments: &[Argument],
        bindings: &Bindings,
    ) -> String {
        let names = self
            .generated
            .values()
            .filter_map(|value| {
                value
                    .semantic_name
                    .as_deref()
                    .map(|name| (value.name.text.as_str(), name))
            })
            .collect::<BTreeMap<_, _>>();
        let mut arguments = arguments.to_vec();
        for argument in &mut arguments {
            if let Argument::Type(value) = argument {
                nominal_names(value, &names, self.catalog);
            }
        }
        canonical::family_key(
            origin,
            &arguments,
            &bindings.parameter_contracts,
            &bindings.argument_identities,
        )
    }

    pub(super) fn checked_alias(
        &mut self,
        template: &TypeSyntax,
        origin: &TypeSyntax,
        semantic_key: &str,
        cache_key: &str,
    ) -> Result<Option<crate::SpannedText>, crate::SyntaxCheckDiagnostic> {
        if self.origins.contains_key(&template.name.text) {
            return Ok(None);
        }
        let mut aliases = self
            .aliases
            .values()
            .filter(|alias| {
                let declaration = self
                    .declarations
                    .iter()
                    .find(|value| value.name == **alias)
                    .expect("Source specialization alias");
                let crate::TypeDefinitionSyntax::Scalar(Type::Reference { value_type, .. }) =
                    &declaration.definition
                else {
                    return false;
                };
                self.generics
                    .get(value_type.text.as_str())
                    .is_some_and(|candidate| core::ptr::eq(*candidate, template))
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Some(preferred) = self.aliases.get(semantic_key) {
            aliases.sort_by_key(|alias| alias != preferred);
        }
        for alias in aliases {
            if !self.alias_checks.insert(alias.text.clone()) {
                continue;
            }
            let declaration = self
                .declarations
                .iter()
                .find(|value| value.name == alias)
                .expect("Source specialization alias");
            let crate::TypeDefinitionSyntax::Scalar(Type::Reference {
                arguments, span, ..
            }) = &declaration.definition
            else {
                unreachable!("Source specialization alias is a family application")
            };
            let arguments = arguments.clone();
            let span = *span;
            let candidate = self.bind(template, &arguments, &Bindings::default(), span);
            self.alias_checks.remove(&alias.text);
            let (resolved, bindings) = candidate?;
            let candidate_semantic_key = self.semantic_key(origin, &resolved, &bindings);
            let candidate_cache_key =
                self.cache_key(template, origin, &candidate_semantic_key, &bindings);
            if candidate_cache_key == cache_key {
                return Ok(Some(alias));
            }
        }
        Ok(None)
    }

    pub(super) fn cache_key(
        &self,
        template: &TypeSyntax,
        origin: &TypeSyntax,
        semantic_key: &str,
        bindings: &Bindings,
    ) -> String {
        if template
            .parameters
            .iter()
            .any(|parameter| parameter.value_type.is_some())
        {
            return semantic_key.into();
        }
        alloc::format!(
            "{semantic_key};preparation={};arguments={:?}",
            canonical::declaration_key(origin, &bindings.parameter_contracts),
            bindings.argument_identities,
        )
    }
}

fn nominal_names(value: &mut Type, names: &BTreeMap<&str, &str>, catalog: &crate::StartupCatalog) {
    match value {
        Type::Reference {
            value_type,
            arguments,
            ..
        } => {
            if let Some(name) = names.get(value_type.text.as_str()) {
                value_type.text = (*name).into();
            } else if let Some(name) = checked_owner_name(&value_type.text, catalog) {
                value_type.text = name;
            }
            for argument in arguments {
                if let Argument::Type(value) = argument {
                    nominal_names(value, names, catalog);
                }
            }
        }
        Type::Optional { value, .. } | Type::DataReference { value, .. } => {
            nominal_names(value, names, catalog)
        }
        Type::Collection { element, .. } | Type::Sequence { element, .. } => {
            nominal_names(element, names, catalog)
        }
    }
}

// Source-owned nominal schemas carry the authored name in their checked identity.
// Only that identity is used; import paths and Rust spellings are not authority.
fn checked_owner_name(name: &str, catalog: &crate::StartupCatalog) -> Option<String> {
    use conduit_core::StructuredInfoTypeShape as Shape;
    let value = catalog.structured_type(name)?;
    let schema = match value.shape() {
        Shape::Nominal { schema, .. }
        | Shape::Record { schema, .. }
        | Shape::Variant { schema, .. } => schema,
        _ => return None,
    };
    let (name, digest) = schema.as_str().strip_prefix("type/")?.rsplit_once('@')?;
    (!name.is_empty() && digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| name.into())
}
