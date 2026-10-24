//! Compiler emission uses only opaque custody retained by the actual checker.
use super::*;
use crate::{CheckedExpression, CheckedSyntaxDocument, KindProjection, ProfileCatalog};
use alloc::collections::BTreeSet;
use conduit_core::{
    KindId, PortTemporal, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape as Shape,
};
impl CheckedSyntaxDocument {
    /// Requested identities select independently retained actual Sources; supplied
    /// packet metadata never becomes an expected Type or family anchor.
    pub fn expression_law_context_for(
        &self,
        identities: &[KindId],
    ) -> Result<LawExpressionContext, LawExpressionRefusal> {
        let mut witnesses = Vec::new();
        let mut seen = BTreeSet::new();
        for identity in identities {
            if !seen.insert(identity.clone()) {
                continue;
            }
            let witness = self
                .native_law_custody
                .values()
                .find(|witness| &witness.checked_type().identity == identity)
                .ok_or(LawExpressionRefusal::Context)?;
            witnesses.push(witness.clone());
        }
        LawExpressionContext::from_source_witnesses(&witnesses)
    }
}
impl ProfileCatalog {
    /// Retain original opaque compiler witnesses supplied independently of the
    /// expanded graph, including imported owner Types. Validate the combined
    /// finite closure before changing the catalogue.
    pub fn install_expression_law_witnesses(
        &mut self,
        witnesses: &[NativeLawWitness],
    ) -> Result<(), LawExpressionRefusal> {
        LawExpressionContext::from_source_witnesses(witnesses)?;
        let mut staged = self.native_law_custody.clone();
        for witness in witnesses {
            let identity = witness.checked_type().identity.as_str();
            if let Some(existing) = staged
                .values()
                .find(|existing| existing.checked_type().identity.as_str() == identity)
            {
                if existing != witness {
                    return Err(LawExpressionRefusal::Context);
                }
            } else {
                staged.insert(alloc::format!("identity:{identity}"), witness.clone());
            }
        }
        let retained = staged.values().cloned().collect::<Vec<_>>();
        LawExpressionContext::from_source_witnesses(&retained)?;
        self.native_law_custody = staged;
        Ok(())
    }

    /// Select the distinct compiler law-capsule profile. Existing bare program
    /// and Kind identities remain available through the default catalogue.
    pub fn enable_checked_native_law_expressions(&mut self) {
        self.checked_native_law_expressions = true;
    }

    pub(crate) fn expression_definition(
        &self,
        expression: &CheckedExpression,
        temporal: PortTemporal,
    ) -> Result<KindProjection, StructuredInfoRefusal> {
        match self.law_material(expression)? {
            Some(material) => law_expression_definition(&material, temporal)
                .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding),
            None => crate::pure_expression_definition(expression, temporal),
        }
    }
    pub(crate) fn filter_definition(
        &self,
        expression: &CheckedExpression,
        temporal: PortTemporal,
    ) -> Result<KindProjection, StructuredInfoRefusal> {
        match self.law_material(expression)? {
            Some(material) => law_filter_definition(&material, temporal)
                .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding),
            None => crate::pure_filter_definition(expression, temporal),
        }
    }
    fn law_material(
        &self,
        expression: &CheckedExpression,
    ) -> Result<Option<LawExpressionMaterial>, StructuredInfoRefusal> {
        if !self.checked_native_law_expressions {
            return Ok(None);
        }
        let malformed = || StructuredInfoRefusal::MalformedCanonicalEncoding;
        let program =
            PortableExpressionProgram::from_checked(expression).map_err(|_| malformed())?;
        if !program.requires_native_law_custody() {
            return Ok(None);
        }
        let mut types = BTreeSet::new();
        let mut remaining = 4096usize;
        collect_type(&program.input_type, &mut types, 0, &mut remaining)?;
        collect_type(&program.output_type, &mut types, 0, &mut remaining)?;
        collect_node(&program.root, &mut types, 0, &mut remaining)?;
        let mut roots = Vec::new();
        let mut included = BTreeSet::new();
        for witness in self.native_law_custody.values() {
            if types.contains(&witness.checked_type().value_type)
                && included.insert(witness.checked_type().identity.clone())
            {
                roots.push(witness.material().map_err(|_| malformed())?);
            }
        }
        // Keep the established declarative artifact when custody is unavailable.
        // Automatic Std realization independently refuses Native bare programs;
        // this preserves exact explicitly supplied custom implementations.
        if roots.is_empty() {
            return Ok(None);
        }
        Ok(Some(LawExpressionMaterial { program, roots }))
    }
}
fn charge(depth: usize, remaining: &mut usize) -> Result<(), StructuredInfoRefusal> {
    if depth > 64 || *remaining == 0 {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    *remaining -= 1;
    Ok(())
}
fn collect_type(
    ty: &StructuredInfoType,
    types: &mut BTreeSet<StructuredInfoType>,
    depth: usize,
    remaining: &mut usize,
) -> Result<(), StructuredInfoRefusal> {
    charge(depth, remaining)?;
    types.insert(ty.clone());
    match ty.shape() {
        Shape::Leaf(_) => (),
        Shape::Nominal { representation, .. } => {
            collect_type(representation, types, depth + 1, remaining)?
        }
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            collect_type(element, types, depth + 1, remaining)?
        }
        Shape::Record { fields, .. } => {
            for field in fields {
                collect_type(field.value_type(), types, depth + 1, remaining)?;
            }
        }
        Shape::Variant { cases, .. } => {
            for case in cases {
                collect_type(case.payload_type(), types, depth + 1, remaining)?;
            }
        }
    }
    Ok(())
}
fn collect_node(
    node: &crate::PortableExpressionNode,
    types: &mut BTreeSet<StructuredInfoType>,
    depth: usize,
    remaining: &mut usize,
) -> Result<(), StructuredInfoRefusal> {
    charge(depth, remaining)?;
    collect_type(&node.value_type, types, depth, remaining)?;
    use crate::PortableExpressionOperation::*;
    match &node.operation {
        Input | Literal(_) | CanonicalLiteral(_) | Constant(_) => (),
        ValueContract { value, .. } | Projection { value, .. } => {
            collect_node(value, types, depth + 1, remaining)?
        }
        Unary { operand, .. } => collect_node(operand, types, depth + 1, remaining)?,
        Binary { left, right, .. } => {
            collect_node(left, types, depth + 1, remaining)?;
            collect_node(right, types, depth + 1, remaining)?;
        }
        Conditional {
            condition,
            when_true,
            when_false,
        } => {
            for child in [condition, when_true, when_false] {
                collect_node(child, types, depth + 1, remaining)?;
            }
        }
        Tuple(items)
        | Collection(items)
        | SemanticCall {
            arguments: items, ..
        } => {
            for item in items {
                collect_node(item, types, depth + 1, remaining)?;
            }
        }
        Record(fields) => {
            for (_, item) in fields {
                collect_node(item, types, depth + 1, remaining)?;
            }
        }
        Variant { payload, .. } => collect_node(payload, types, depth + 1, remaining)?,
    }
    Ok(())
}
