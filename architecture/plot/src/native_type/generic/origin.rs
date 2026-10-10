//! Family meaning resolves owner references independently of lookup aliases.
use super::{binding::type_span, canonical, error, parameter, Context};
use crate::prelude::*;
use crate::{
    NativeTypeArgumentSyntax as Argument, StartupCatalog, SyntaxCheckDiagnostic,
    TypeDefinitionSyntax as Definition, TypeExpressionSyntax as Type, TypeSyntax,
    TypeVariantPayloadSyntax as Payload,
};
use alloc::collections::{BTreeMap, BTreeSet};
use sha2::{Digest, Sha256};

#[derive(Default)]
struct OriginWork {
    active: BTreeSet<String>,
    meanings: BTreeMap<String, String>,
    steps: usize,
}

impl Context<'_> {
    pub(super) fn semantic_origin(
        &self,
        template: &TypeSyntax,
    ) -> Result<TypeSyntax, SyntaxCheckDiagnostic> {
        let mut references = Vec::new();
        super::super::definition_references(&template.definition, &mut references);
        for parameter in &template.parameters {
            if let Some(annotation) = &parameter.value_type {
                super::super::expression_references(annotation, &mut references);
            }
        }
        references.retain(|name| {
            !template
                .parameters
                .iter()
                .any(|parameter| parameter.name.text == *name)
        });
        let declarations = self
            .declarations
            .iter()
            .chain(self.generated.values())
            .cloned()
            .collect::<Vec<_>>();
        let catalog = parameter::prepare_references(&references, &declarations, self.catalog)
            .map_err(|diagnostic| {
                if diagnostic
                    .message
                    .starts_with("native parameter Type dependencies are recursive")
                {
                    error(
                        diagnostic.span,
                        "recursive or unbounded generic semantic Type dependency meaning".into(),
                    )
                } else {
                    diagnostic
                }
            })?;
        self.normalize_origin(template, &catalog, &mut OriginWork::default())
    }
    fn normalize_origin(
        &self,
        template: &TypeSyntax,
        catalog: &StartupCatalog,
        work: &mut OriginWork,
    ) -> Result<TypeSyntax, SyntaxCheckDiagnostic> {
        if work.active.len() >= 32 || !work.active.insert(template.name.text.clone()) {
            return Err(error(
                template.name.span,
                "recursive or unbounded family dependency meaning".into(),
            ));
        }
        let mut origin = template.clone();
        origin
            .name
            .text
            .clone_from(&self.origin(template).name.text);
        let parameters = template
            .parameters
            .iter()
            .map(|parameter| parameter.name.text.as_str())
            .collect::<BTreeSet<_>>();
        self.origin_definition(&mut origin.definition, &parameters, catalog, work)?;
        for parameter in &mut origin.parameters {
            if let Some(annotation) = &mut parameter.value_type {
                self.origin_expression(annotation, &BTreeSet::new(), catalog, work)?;
            }
        }
        work.active.remove(&template.name.text);
        Ok(origin)
    }
    fn origin_definition(
        &self,
        value: &mut Definition,
        parameters: &BTreeSet<&str>,
        catalog: &StartupCatalog,
        work: &mut OriginWork,
    ) -> Result<(), SyntaxCheckDiagnostic> {
        match value {
            Definition::Quantity(_) => {}
            Definition::Scalar(value) => {
                self.origin_expression(value, parameters, catalog, work)?
            }
            Definition::Record(fields) => {
                for field in fields {
                    self.origin_expression(&mut field.value_type, parameters, catalog, work)?;
                }
            }
            Definition::Variant(cases) => {
                for case in cases {
                    match &mut case.payload {
                        Payload::Empty => {}
                        Payload::Type(value) => {
                            self.origin_expression(value, parameters, catalog, work)?
                        }
                        Payload::Record(fields) => {
                            for field in fields {
                                self.origin_expression(
                                    &mut field.value_type,
                                    parameters,
                                    catalog,
                                    work,
                                )?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn origin_expression(
        &self,
        value: &mut Type,
        parameters: &BTreeSet<&str>,
        catalog: &StartupCatalog,
        work: &mut OriginWork,
    ) -> Result<(), SyntaxCheckDiagnostic> {
        let span = type_span(value);
        work.steps = work.steps.saturating_add(1);
        if work.steps > 4096 {
            return Err(error(
                span,
                "native family dependency meaning exceeds its finite work budget".into(),
            ));
        }
        match value {
            Type::Reference {
                value_type,
                arguments,
                ..
            } => {
                if !parameters.contains(value_type.text.as_str()) {
                    if let Some(template) = self.generics.get(value_type.text.as_str()) {
                        let meaning = if let Some(meaning) = work.meanings.get(&template.name.text)
                        {
                            meaning.clone()
                        } else {
                            let normalized = self.normalize_origin(template, catalog, work)?;
                            let declaration = canonical::declaration_key(&normalized, &[]);
                            let meaning = alloc::format!(
                                "family:{}:{:x}",
                                normalized.name.text,
                                Sha256::digest(declaration.as_bytes())
                            );
                            work.meanings
                                .insert(template.name.text.clone(), meaning.clone());
                            meaning
                        };
                        value_type.text = meaning;
                    } else {
                        let checked =
                            crate::value_type::checked_value_type(&value_type.text, catalog)
                                .map_err(|_| {
                                    error(
                                        span,
                                        alloc::format!(
                                            "semantic family dependency '{}' is not in scope",
                                            value_type.text
                                        ),
                                    )
                                })?;
                        let bytes = checked.canonical_bytes().map_err(|refusal| {
                            error(
                                span,
                                alloc::format!("invalid family dependency: {refusal:?}"),
                            )
                        })?;
                        value_type.text = alloc::format!("type:{:x}", Sha256::digest(&bytes));
                    }
                }
                for argument in arguments {
                    if let Argument::Type(value) = argument {
                        self.origin_expression(value, parameters, catalog, work)?;
                    }
                }
            }
            Type::Optional { value, .. } | Type::DataReference { value, .. } => {
                self.origin_expression(value, parameters, catalog, work)?
            }
            Type::Collection { element, .. } | Type::Sequence { element, .. } => {
                self.origin_expression(element, parameters, catalog, work)?
            }
        }
        Ok(())
    }
}
