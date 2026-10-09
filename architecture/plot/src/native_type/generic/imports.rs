//! Lower owner-captured families into private preparation names.
use super::{budget, error};
use crate::prelude::*;
use crate::{
    NativeTypeArgumentSyntax as Argument, StartupCatalog, SyntaxCheckDiagnostic,
    TypeDefinitionSyntax as Definition, TypeExpressionSyntax as Type, TypeSyntax,
    TypeVariantPayloadSyntax as Payload,
};
use alloc::collections::{BTreeMap, BTreeSet};

pub(super) struct Imports {
    pub(super) catalog: StartupCatalog,
    pub(super) templates: Vec<TypeSyntax>,
    pub(super) origins: BTreeMap<String, TypeSyntax>,
    pub(super) aliases: BTreeMap<String, String>,
}
impl Imports {
    pub(super) fn prepare(catalog: &StartupCatalog) -> Result<Self, SyntaxCheckDiagnostic> {
        let mut imports = Self {
            catalog: catalog.clone(),
            templates: Vec::new(),
            origins: BTreeMap::new(),
            aliases: BTreeMap::new(),
        };
        for (alias, family) in &catalog.native_families {
            budget::validate(&family.templates)?;
            let prefix = family
                .package_content_digest
                .iter()
                .map(|byte| alloc::format!("{byte:02x}"))
                .collect::<String>();
            let names = family
                .templates
                .iter()
                .map(|value| value.name.text.as_str())
                .chain(family.dependencies.iter().map(|value| value.name.as_str()))
                .map(|name| {
                    (
                        name.to_string(),
                        alloc::format!("__native_family_{prefix}_{name}"),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for dependency in &family.dependencies {
                let name = &names[&dependency.name];
                if let Some(existing) = imports.catalog.structured_type(name) {
                    if existing == &dependency.value_type {
                        continue;
                    }
                }
                imports
                    .catalog
                    .insert_checked_native_type(name.clone(), dependency)
                    .map_err(|message| error(family.templates[0].name.span, message))?;
            }
            for original in &family.templates {
                let name = &names[&original.name.text];
                if imports.origins.contains_key(name) {
                    continue;
                }
                let mut template = original.clone();
                template.name.text.clone_from(name);
                let parameters = template
                    .parameters
                    .iter()
                    .map(|value| value.name.text.clone())
                    .collect::<BTreeSet<_>>();
                definition(&mut template.definition, &names, &parameters);
                for parameter in &mut template.parameters {
                    if let Some(annotation) = &mut parameter.value_type {
                        expression(annotation, &names, &BTreeSet::new());
                    }
                }
                imports.origins.insert(name.clone(), original.clone());
                imports.templates.push(template);
            }
            imports
                .aliases
                .insert(alias.clone(), names[&family.root].clone());
        }
        budget::validate(&imports.templates)?;
        Ok(imports)
    }
}
fn definition(
    value: &mut Definition,
    names: &BTreeMap<String, String>,
    parameters: &BTreeSet<String>,
) {
    match value {
        Definition::Scalar(value) => expression(value, names, parameters),
        Definition::Record(fields) => {
            for field in fields {
                expression(&mut field.value_type, names, parameters);
            }
        }
        Definition::Variant(cases) => {
            for case in cases {
                match &mut case.payload {
                    Payload::Unit => {}
                    Payload::Type(value) => expression(value, names, parameters),
                    Payload::Record(fields) => {
                        for field in fields {
                            expression(&mut field.value_type, names, parameters);
                        }
                    }
                }
            }
        }
    }
}
fn expression(value: &mut Type, names: &BTreeMap<String, String>, parameters: &BTreeSet<String>) {
    match value {
        Type::Reference {
            value_type,
            arguments,
            ..
        } => {
            if !parameters.contains(&value_type.text) {
                if let Some(name) = names.get(&value_type.text) {
                    value_type.text.clone_from(name);
                }
            }
            for argument in arguments {
                if let Argument::Type(value) = argument {
                    expression(value, names, parameters);
                }
            }
        }
        Type::Optional { value, .. } | Type::DataReference { value, .. } => {
            expression(value, names, parameters)
        }
        Type::Collection { element, .. } | Type::Sequence { element, .. } => {
            expression(element, names, parameters)
        }
    }
}

#[cfg(test)]
mod tests;
