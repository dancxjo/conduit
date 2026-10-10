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
                nominal_names(value, &names);
            }
        }
        canonical::family_key(
            origin,
            &arguments,
            &bindings.parameter_contracts,
            &bindings.argument_identities,
        )
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

fn nominal_names(value: &mut Type, names: &BTreeMap<&str, &str>) {
    match value {
        Type::Reference {
            value_type,
            arguments,
            ..
        } => {
            if let Some(name) = names.get(value_type.text.as_str()) {
                value_type.text = (*name).into();
            }
            for argument in arguments {
                if let Argument::Type(value) = argument {
                    nominal_names(value, names);
                }
            }
        }
        Type::Optional { value, .. } | Type::DataReference { value, .. } => {
            nominal_names(value, names)
        }
        Type::Collection { element, .. } | Type::Sequence { element, .. } => {
            nominal_names(element, names)
        }
    }
}
