use std::{collections::BTreeMap, fmt, path::Path};

use sha2::{Digest as _, Sha256};

use crate::{Panel, parse_module, parse_with_root};

/// UTF-8 source returned by an explicitly supplied module loader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedModule {
    /// Canonical URI actually loaded. It must equal the requested URI.
    pub canonical_uri: String,
    /// Complete UTF-8 `.panel` source.
    pub source: String,
}

/// Explicit module source. The resolver itself performs no filesystem or
/// network I/O.
pub trait ModuleLoader {
    /// Loads one already normalized URI.
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String>;
}

/// One parsed, content-identified module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModule {
    pub canonical_uri: String,
    pub content_hash: String,
    pub panel: Panel,
}

/// Complete deterministic import closure in dependency-first order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleGraph {
    pub entry_uri: String,
    pub selected_root: Option<String>,
    pub modules: Vec<ResolvedModule>,
}

/// Stable module/import failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleResolutionError {
    pub code: &'static str,
    pub uri: String,
    pub import_chain: Vec<String>,
    pub message: String,
}

impl fmt::Display for ModuleResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} for {} via {}: {}",
            self.code,
            self.uri,
            self.import_chain.join(" -> "),
            self.message
        )
    }
}

impl std::error::Error for ModuleResolutionError {}

/// Resolves one explicit import closure without implicit I/O.
///
/// The caller supplies the loader and optional entry-root selection. Relative
/// targets are normalized against the importing module before the loader is
/// called. Every loaded byte string is SHA-256 identified.
pub fn resolve_modules(
    entry_uri: &str,
    selected_root: Option<&str>,
    loader: &impl ModuleLoader,
) -> Result<ModuleGraph, ModuleResolutionError> {
    let entry_uri = normalize_uri(entry_uri).map_err(|message| ModuleResolutionError {
        code: "CND-SRC-003",
        uri: entry_uri.to_owned(),
        import_chain: vec![entry_uri.to_owned()],
        message,
    })?;
    let mut resolver = Resolver {
        loader,
        visiting: Vec::new(),
        resolved: BTreeMap::new(),
        order: Vec::new(),
    };
    resolver.visit(&entry_uri, None, true, selected_root)?;
    let selected_root = resolver
        .resolved
        .get(&entry_uri)
        .and_then(|module| module.panel.selected_root.clone());
    let modules = resolver
        .order
        .iter()
        .map(|uri| resolver.resolved[uri].clone())
        .collect();
    Ok(ModuleGraph {
        entry_uri,
        selected_root,
        modules,
    })
}

struct Resolver<'a, L> {
    loader: &'a L,
    visiting: Vec<String>,
    resolved: BTreeMap<String, ResolvedModule>,
    order: Vec<String>,
}

impl<L: ModuleLoader> Resolver<'_, L> {
    fn visit(
        &mut self,
        uri: &str,
        expected_hash: Option<&str>,
        entry: bool,
        selected_root: Option<&str>,
    ) -> Result<(), ModuleResolutionError> {
        if let Some(position) = self.visiting.iter().position(|candidate| candidate == uri) {
            let mut cycle = self.visiting[position..].to_vec();
            cycle.push(uri.to_owned());
            return Err(self.error(
                "CND-SRC-004",
                uri,
                format!("import cycle: {}", cycle.join(" -> ")),
            ));
        }
        if let Some(module) = self.resolved.get(uri) {
            verify_expected_hash(uri, expected_hash, &module.content_hash, &self.visiting)?;
            return Ok(());
        }

        self.visiting.push(uri.to_owned());
        let loaded = self
            .loader
            .load(uri)
            .map_err(|message| self.error("CND-SRC-003", uri, message))?
            .ok_or_else(|| self.error("CND-SRC-003", uri, "module is absent"))?;
        if loaded.canonical_uri != uri {
            return Err(self.error(
                "CND-SRC-003",
                uri,
                format!(
                    "loader returned non-matching canonical URI `{}`",
                    loaded.canonical_uri
                ),
            ));
        }
        let content_hash = content_hash(&loaded.source);
        verify_expected_hash(uri, expected_hash, &content_hash, &self.visiting)?;
        let panel = if entry {
            parse_with_root(&loaded.source, selected_root)
        } else {
            parse_module(&loaded.source)
        }
        .map_err(|error| {
            self.error(
                error.code,
                uri,
                format!(
                    "source diagnostic at {}:{}: {}",
                    error.line, error.column, error.message
                ),
            )
        })?;

        for import in &panel.imports {
            let imported_uri = resolve_import_uri(uri, &import.target)
                .map_err(|message| self.error("CND-SRC-003", uri, message))?;
            self.visit(&imported_uri, import.content_hash.as_deref(), false, None)?;
        }
        validate_qualified_symbols(uri, &panel, &self.resolved, &self.visiting)?;

        self.visiting.pop();
        self.order.push(uri.to_owned());
        self.resolved.insert(
            uri.to_owned(),
            ResolvedModule {
                canonical_uri: uri.to_owned(),
                content_hash,
                panel,
            },
        );
        Ok(())
    }

    fn error(
        &self,
        code: &'static str,
        uri: &str,
        message: impl Into<String>,
    ) -> ModuleResolutionError {
        ModuleResolutionError {
            code,
            uri: uri.to_owned(),
            import_chain: self.visiting.clone(),
            message: message.into(),
        }
    }
}

fn verify_expected_hash(
    uri: &str,
    expected: Option<&str>,
    actual: &str,
    chain: &[String],
) -> Result<(), ModuleResolutionError> {
    if let Some(expected) = expected {
        if expected != actual {
            return Err(ModuleResolutionError {
                code: "CND-SRC-005",
                uri: uri.to_owned(),
                import_chain: chain.to_vec(),
                message: format!("content pin differs: expected {expected}, actual {actual}"),
            });
        }
    }
    Ok(())
}

fn validate_qualified_symbols(
    uri: &str,
    panel: &Panel,
    resolved: &BTreeMap<String, ResolvedModule>,
    chain: &[String],
) -> Result<(), ModuleResolutionError> {
    let imports: BTreeMap<&str, (&crate::Import, String)> = panel
        .imports
        .iter()
        .map(|import| {
            resolve_import_uri(uri, &import.target)
                .map(|target| (import.alias.as_str(), (import, target)))
        })
        .collect::<Result<_, _>>()
        .map_err(|message| ModuleResolutionError {
            code: "CND-SRC-003",
            uri: uri.to_owned(),
            import_chain: chain.to_vec(),
            message,
        })?;
    for node in panel.nodes.iter().chain(
        panel
            .definitions
            .iter()
            .flat_map(|definition| &definition.nodes),
    ) {
        let Some((alias, symbol)) = node.kind.split_once('.') else {
            continue;
        };
        let Some((_, imported_uri)) = imports.get(alias) else {
            continue;
        };
        let imported = &resolved[imported_uri];
        if !imported
            .panel
            .definitions
            .iter()
            .any(|definition| definition.id == symbol)
        {
            return Err(ModuleResolutionError {
                code: "CND-SRC-003",
                uri: uri.to_owned(),
                import_chain: chain.to_vec(),
                message: format!(
                    "qualified symbol `{}` is absent from import `{alias}`",
                    node.kind
                ),
            });
        }
    }
    Ok(())
}

fn content_hash(source: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(source.as_bytes()))
}

fn resolve_import_uri(base: &str, target: &str) -> Result<String, String> {
    if target.contains("://") || target.starts_with('/') {
        return normalize_uri(target);
    }
    if let Some((prefix, path)) = split_uri_path(base) {
        let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        return normalize_uri(&format!("{prefix}{parent}/{target}"));
    }
    let parent = Path::new(base)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or_default();
    normalize_uri(&format!("{parent}/{target}"))
}

fn normalize_uri(uri: &str) -> Result<String, String> {
    if uri.is_empty() || uri.contains(['\0', '\\']) {
        return Err("URI is empty or contains a non-portable character".to_owned());
    }
    let (prefix, path) = split_uri_path(uri).unwrap_or(("", uri));
    let absolute = path.starts_with('/');
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!("URI escapes its root: {uri}"));
                }
            }
            value => parts.push(value),
        }
    }
    let slash = if absolute { "/" } else { "" };
    Ok(format!("{prefix}{slash}{}", parts.join("/")))
}

fn split_uri_path(uri: &str) -> Option<(&str, &str)> {
    let scheme = uri.find("://")?;
    let after_scheme = scheme + 3;
    let Some(path) = uri[after_scheme..].find('/') else {
        return Some((uri, ""));
    };
    Some(uri.split_at(path + after_scheme))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemoryLoader(BTreeMap<String, String>);

    impl ModuleLoader for MemoryLoader {
        fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
            Ok(self.0.get(canonical_uri).map(|source| LoadedModule {
                canonical_uri: canonical_uri.to_owned(),
                source: source.clone(),
            }))
        }
    }

    #[test]
    fn relative_imports_are_dependency_ordered_and_content_identified() {
        let child = "panel 1\nnode worker { node value : conduit/literal }\nroot worker\n";
        let pin = content_hash(child);
        let root = format!(
            "panel 1\nimport \"./child.panel\" as child pin \"{pin}\"\n\
             node app {{ node worker : child.worker }}\nroot app\n"
        );
        let loader = MemoryLoader(BTreeMap::from([
            ("mem://fixture/root.panel".to_owned(), root),
            ("mem://fixture/child.panel".to_owned(), child.to_owned()),
        ]));
        let graph = resolve_modules("mem://fixture/root.panel", None, &loader).unwrap();
        assert_eq!(graph.modules.len(), 2);
        assert_eq!(graph.modules[0].canonical_uri, "mem://fixture/child.panel");
        assert_eq!(graph.selected_root.as_deref(), Some("app"));
    }

    #[test]
    fn cycles_report_the_complete_import_chain() {
        let loader = MemoryLoader(BTreeMap::from([
            (
                "mem://fixture/a.panel".to_owned(),
                "panel 1\nimport \"./b.panel\" as b\n".to_owned(),
            ),
            (
                "mem://fixture/b.panel".to_owned(),
                "panel 1\nimport \"./a.panel\" as a\n".to_owned(),
            ),
        ]));
        let error = resolve_modules("mem://fixture/a.panel", None, &loader).unwrap_err();
        assert_eq!(error.code, "CND-SRC-004");
        assert!(error.message.contains("a.panel"));
        assert!(error.message.contains("b.panel"));
    }

    #[test]
    fn absolute_uris_are_not_rebased_to_the_importer() {
        let loader = MemoryLoader(BTreeMap::from([
            (
                "mem://entry/root.panel".to_owned(),
                "panel 1\nimport \"mem://shared/child.panel\" as child\n".to_owned(),
            ),
            (
                "mem://shared/child.panel".to_owned(),
                "panel 1\n".to_owned(),
            ),
        ]));
        let graph = resolve_modules("mem://entry/root.panel", None, &loader).unwrap();
        assert_eq!(graph.modules[0].canonical_uri, "mem://shared/child.panel");
    }
}
