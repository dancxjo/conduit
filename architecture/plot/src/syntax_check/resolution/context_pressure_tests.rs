//! Real immutable Source aliases exercise glyph-context admission pressure.
use super::*;
use crate::*;

fn resolve(payload_bytes: usize, aliases: usize) -> Result<usize, SyntaxCheckError> {
    resolve_with_chain(payload_bytes, aliases, false)
}

fn resolve_with_chain(
    payload_bytes: usize,
    aliases: usize,
    chain: bool,
) -> Result<usize, SyntaxCheckError> {
    let declarations = (0..aliases)
        .map(|i| {
            format!(
                "copy-{i} = {}\n",
                if chain && i > 0 {
                    format!("copy-{}", i - 1)
                } else {
                    "base".into()
                }
            )
        })
        .collect::<String>();
    let source = format!("type Packet = {{\n payload: Text <= 32768B\n}}\nplot example {{\nbase = {{payload: \"{}\"}}\n{declarations}}}\n", "x".repeat(payload_bytes));
    let document = parse_syntax_document(&source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    // Obtain the actual checked Native Type without trying to infer record locals.
    let definitions = source.split("plot example").next().unwrap();
    let checked =
        check_syntax_document(&parse_syntax_document(definitions), &StartupCatalog::new()).unwrap();
    let ty = &checked.native_types[0].value_type;
    let locals = document.plots[0]
        .back
        .iter()
        .map(|statement| {
            let BackStatement::LocalValue(local) = statement else {
                panic!()
            };
            (local.name.text.clone(), local)
        })
        .collect();
    let startup = StartupCatalog::new();
    let mut resolver = Resolver::new(
        locals,
        BTreeMap::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        &startup,
    );
    resolver.bound_glyph_context();
    if chain && aliases > 0 {
        resolver.resolve_name(&format!("copy-{}", aliases - 1), Some(ty))?;
    }
    resolver.resolve_name("base", Some(ty))?;
    for index in 0..aliases {
        resolver.resolve_name(&format!("copy-{index}"), Some(ty))?;
    }
    Ok(resolver.resolved_context().unwrap().len())
}
#[test]
fn immutable_context_entry_limit_admits_64_and_refuses_65() {
    assert_eq!(resolve(1, 63).unwrap(), 64);
    let error = resolve(1, 64)
        .unwrap_err()
        .diagnostic(crate::whole_source_span(""));
    assert!(error.message.contains("64 entries"), "{error:?}");
}
#[test]
fn canonical_alias_storage_refuses_even_when_authored_source_is_small() {
    assert_eq!(resolve(32768, 16).unwrap(), 17);
    let error = resolve(32768, 40)
        .unwrap_err()
        .diagnostic(crate::whole_source_span(""));
    assert!(error.message.contains("1 MiB"), "{error:?}");
}

#[test]
fn cold_alias_chain_admits_maximum_depth_and_refuses_the_next_dependency() {
    assert_eq!(resolve_with_chain(1, 63, true).unwrap(), 64);
    let error = resolve_with_chain(1, 64, true)
        .unwrap_err()
        .diagnostic(crate::whole_source_span(""));
    assert!(
        error.message.contains("dependency depth exceeds 64"),
        "{error:?}"
    );
}
