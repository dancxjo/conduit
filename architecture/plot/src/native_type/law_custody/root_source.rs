//! Retain exact declaration slices selected by compiler syntax dependencies.
use super::*;
use alloc::collections::BTreeSet;

pub(super) fn extract(
    syntax: &crate::SyntaxDocument,
    root: &str,
) -> Result<String, NativeLawCustodyRefusal> {
    if syntax.types.len() > 4096 || syntax.uses.len() > 4096 {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    let mut selected = BTreeSet::new();
    let mut imports = BTreeSet::new();
    visit(syntax, root, 0, &mut selected, &mut imports)?;
    let mut spans = Vec::new();
    for declaration in &syntax.uses {
        if imports.contains(declaration.alias.text.as_str()) {
            spans.push(declaration.span);
        }
    }
    for declaration in &syntax.types {
        if selected.contains(declaration.name.text.as_str()) {
            spans.push(declaration.span);
        }
    }
    spans.sort_by_key(|span| span.start);
    let mut length = 0usize;
    let mut end = 0;
    for span in &spans {
        if span.start < end {
            return Err(NativeLawCustodyRefusal::Source);
        }
        let piece = syntax
            .round_trip()
            .get(span.start..span.end)
            .ok_or(NativeLawCustodyRefusal::Source)?;
        length = length
            .checked_add(piece.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(NativeLawCustodyRefusal::Capacity)?;
        if length > MAXIMUM_BYTES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        end = span.end;
    }
    let mut source = String::with_capacity(length);
    for span in spans {
        source.push_str(&syntax.round_trip()[span.start..span.end]);
        source.push('\n');
    }
    Ok(source)
}
fn visit<'a>(
    syntax: &'a crate::SyntaxDocument,
    name: &str,
    depth: usize,
    selected: &mut BTreeSet<&'a str>,
    imports: &mut BTreeSet<&'a str>,
) -> Result<(), NativeLawCustodyRefusal> {
    if depth > 32 {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    let declaration = syntax
        .types
        .iter()
        .find(|ty| ty.name.text == name)
        .ok_or(NativeLawCustodyRefusal::Declaration)?;
    if !selected.insert(declaration.name.text.as_str()) {
        return Ok(());
    }
    let mut references = Vec::new();
    super::super::definition_references(&declaration.definition, &mut references);
    for parameter in &declaration.parameters {
        if let Some(annotation) = &parameter.value_type {
            super::super::expression_references(annotation, &mut references);
        }
    }
    for reference in references {
        if declaration
            .parameters
            .iter()
            .any(|p| p.name.text == reference)
        {
            continue;
        }
        if syntax.types.iter().any(|ty| ty.name.text == reference) {
            visit(syntax, reference, depth + 1, selected, imports)?;
        } else if syntax.uses.iter().any(|u| u.alias.text == reference) {
            imports.insert(reference);
        }
    }
    Ok(())
}

/// Bundle checking already gives Native declarations a common lexical scope.
/// Reproduce only their exact fragments; final per-root checking still proves
/// the extracted closure has the original expected identity and laws.
pub(super) fn merge_documents(
    documents: &[crate::SyntaxDocument],
) -> Result<String, NativeLawCustodyRefusal> {
    if documents.len() > 256 {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    let mut fragments = Vec::new();
    let mut uses = BTreeSet::new();
    let mut length = 0usize;
    for document in documents {
        let mut spans = Vec::new();
        for used in &document.uses {
            if uses.insert((used.path.as_str(), used.alias.text.as_str())) {
                spans.push(used.span);
            }
        }
        spans.extend(document.types.iter().map(|ty| ty.span));
        spans.sort_by_key(|span| span.start);
        for span in spans {
            let fragment = document
                .round_trip()
                .get(span.start..span.end)
                .ok_or(NativeLawCustodyRefusal::Source)?;
            length = length
                .checked_add(fragment.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(NativeLawCustodyRefusal::Capacity)?;
            if length > MAXIMUM_BYTES {
                return Err(NativeLawCustodyRefusal::Capacity);
            }
            fragments.push(fragment);
        }
    }
    let mut source = String::with_capacity(length);
    for fragment in fragments {
        source.push_str(fragment);
        source.push('\n');
    }
    Ok(source)
}
