//! Declaration-directed Type and compile-time Info argument binding.
use super::{error, integer, Context};
use crate::prelude::*;
use crate::{
    NativeIntegerExpressionSyntax as Integer, NativeTypeArgumentSyntax as Argument, Span,
    SyntaxCheckDiagnostic, TypeExpressionSyntax, TypeSyntax,
};
use alloc::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Bindings {
    pub(super) types: BTreeMap<String, TypeExpressionSyntax>,
    pub(super) values: BTreeMap<String, u16>,
}

impl Context<'_> {
    pub(super) fn bind(
        &mut self,
        template: &TypeSyntax,
        arguments: &[Argument],
        outer: &Bindings,
        span: Span,
    ) -> Result<(Vec<Argument>, Bindings), SyntaxCheckDiagnostic> {
        if template.parameters.len() != arguments.len() {
            return Err(error(
                span,
                alloc::format!(
                    "generic semantic Type '{}' expects {} arguments but received {}",
                    template.name.text,
                    template.parameters.len(),
                    arguments.len()
                ),
            ));
        }
        if arguments.len() > 16 {
            return Err(error(
                span,
                "native Type family exceeds its 16-argument profile".into(),
            ));
        }
        let mut resolved = Vec::new();
        let mut bindings = Bindings::default();
        for (parameter, argument) in template.parameters.iter().zip(arguments) {
            if let Some(annotation) = &parameter.value_type {
                let value = match argument {
                    Argument::Value(value) => integer::normalize(value, &outer.values)?,
                    Argument::Type(value) => {
                        let TypeExpressionSyntax::Reference {
                            value_type,
                            arguments,
                            maximum_bytes: None,
                            refinements,
                            ..
                        } = value.as_ref()
                        else {
                            return Err(error(
                                span,
                                "an Info parameter requires a finite integer argument".into(),
                            ));
                        };
                        if !arguments.is_empty() || !refinements.is_empty() {
                            return Err(error(
                                value_type.span,
                                "an Info parameter requires a finite integer argument".into(),
                            ));
                        }
                        integer::normalize(&Integer::Parameter(value_type.clone()), &outer.values)?
                    }
                };
                let scalar = value.literal_value().expect("normalized integer");
                let compiled = super::super::compile_expression(annotation, self.catalog)?;
                if super::super::primitive_representation_kind(&compiled.value_type)
                    .map(|kind| kind.as_str())
                    != Some("value/u16")
                {
                    return Err(error(parameter.name.span, "the first native Info parameter profile requires U16 or a checked U16 refinement".into()));
                }
                for contract in &compiled.contracts {
                    if !contract.representation_path.is_empty() {
                        return Err(error(
                            parameter.name.span,
                            "a native Info parameter requires a scalar contract".into(),
                        ));
                    }
                    contract.contract.validate(&scalar.to_le_bytes()).map_err(|refusal| error(value.span(), alloc::format!("native Info argument violates its declared scalar contract: {refusal:?}")))?;
                }
                bindings.values.insert(parameter.name.text.clone(), scalar);
                resolved.push(Argument::Value(Box::new(value)));
            } else {
                let Argument::Type(value) = argument else {
                    let Argument::Value(value) = argument else {
                        unreachable!()
                    };
                    return Err(error(
                        value.span(),
                        "a Type parameter cannot accept an Info argument".into(),
                    ));
                };
                if let TypeExpressionSyntax::Reference {
                    value_type,
                    arguments,
                    ..
                } = value.as_ref()
                {
                    if arguments.is_empty() && outer.values.contains_key(&value_type.text) {
                        return Err(error(
                            value_type.span,
                            "a Type parameter cannot accept an Info parameter".into(),
                        ));
                    }
                }
                let value = self.expression(value, outer)?;
                bindings
                    .types
                    .insert(parameter.name.text.clone(), value.clone());
                resolved.push(Argument::Type(Box::new(value)));
            }
        }
        Ok((resolved, bindings))
    }
}

#[cfg(test)]
mod tests;

pub(super) fn type_span(value: &TypeExpressionSyntax) -> Span {
    match value {
        TypeExpressionSyntax::Reference { span, .. }
        | TypeExpressionSyntax::Optional { span, .. }
        | TypeExpressionSyntax::DataReference { span, .. }
        | TypeExpressionSyntax::Collection { span, .. }
        | TypeExpressionSyntax::Sequence { span, .. } => *span,
    }
}
