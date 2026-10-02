use conduit_plot::{SourceSugarExpansion, Span};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct ExpansionReport<'a> {
    schema: &'static str,
    boundary: &'static str,
    source_document_id: &'a str,
    expansions: Vec<ExpansionView<'a>>,
}

#[derive(Debug, Serialize)]
struct ExpansionView<'a> {
    plot: &'a str,
    checked_plot_id: &'a str,
    authored: &'a str,
    source_span: SourceSpan,
    ordinary_kind: &'a str,
    input_ports: &'a [String],
    output_ports: &'a [String],
    operand_bindings: Vec<OperandBinding<'a>>,
    canonical_replacement: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct OperandBinding<'a> {
    source: &'a str,
    input_port: &'a str,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct SourceSpan {
    start: usize,
    end: usize,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
}

impl From<Span> for SourceSpan {
    fn from(span: Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
            line: span.line,
            column: span.column,
            end_line: span.end_line,
            end_column: span.end_column,
        }
    }
}

pub(crate) fn run(path: &Path, json: bool) -> Result<String, String> {
    let source = crate::plot_source::load(path)?;
    let checked = source.check()?;
    let report = ExpansionReport {
        schema: "conduit.source-sugar-expansion@1",
        boundary: "authored source sugar; canonical checked Plot remains authoritative",
        source_document_id: checked.source_document_id.as_str(),
        expansions: checked.source_sugar_expansions.iter().map(view).collect(),
    };
    if json {
        serde_json::to_string_pretty(&report)
            .map(|mut rendered| {
                rendered.push('\n');
                rendered
            })
            .map_err(|error| error.to_string())
    } else {
        Ok(render_human(&report))
    }
}

fn view(expansion: &SourceSugarExpansion) -> ExpansionView<'_> {
    ExpansionView {
        plot: &expansion.plot,
        checked_plot_id: expansion.checked_plot_id.as_str(),
        authored: &expansion.authored,
        source_span: expansion.source_span.into(),
        ordinary_kind: &expansion.ordinary_kind,
        input_ports: &expansion.input_ports,
        output_ports: &expansion.output_ports,
        operand_bindings: expansion
            .operand_bindings
            .iter()
            .map(|binding| OperandBinding {
                source: &binding.source,
                input_port: &binding.input_port,
            })
            .collect(),
        canonical_replacement: expansion.canonical_replacement.as_deref(),
    }
}

fn render_human(report: &ExpansionReport<'_>) -> String {
    let mut output =
        String::from("Authored source sugar (the canonical checked Plot remains authoritative)\n");
    if report.expansions.is_empty() {
        output.push_str("No admitted concise source spelling occurs.\n");
        return output;
    }
    for expansion in &report.expansions {
        output.push_str(&format!(
            "\n{}:{} `{}` in plot `{}`\n  ordinary Gear: {}\n  checked Plot: {}\n  Fore: ({}) -> ({})\n",
            expansion.source_span.line,
            expansion.source_span.column,
            expansion.authored,
            expansion.plot,
            expansion.ordinary_kind,
            expansion.checked_plot_id,
            expansion.input_ports.join(", "),
            expansion.output_ports.join(", "),
        ));
        if let Some(replacement) = expansion.canonical_replacement {
            output.push_str(&format!("  lossless stage spelling: {replacement}\n"));
        }
        for binding in &expansion.operand_bindings {
            output.push_str(&format!(
                "  operand `{}` -> `{}`\n",
                binding.source, binding.input_port
            ));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_view_names_the_semantic_boundary() {
        let report = ExpansionReport {
            schema: "conduit.source-sugar-expansion@1",
            boundary: "authored source sugar; canonical checked Plot remains authoritative",
            source_document_id: "source",
            expansions: Vec::new(),
        };
        let rendered = render_human(&report);
        assert!(rendered.contains("canonical checked Plot remains authoritative"));
        assert!(rendered.contains("No admitted concise source spelling"));
    }

    #[test]
    fn installed_catalog_drives_both_json_and_human_views() {
        let path = std::env::temp_dir().join(format!(
            "conduit-source-expansion-{}.conduit",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "sans glyphs\nwith pair as &>\nplot pair (\n  >> left: Text\n  >> right: Text\n  paired: Text >>\n) {\n}\nplot example (\n  >> left: Text\n  >> right: Text\n  paired: Text >>\n) {\n  left &> right >> paired\n}\n",
        )
        .unwrap();

        let human = run(&path, false).unwrap();
        let json = run(&path, true).unwrap();
        std::fs::remove_file(path).unwrap();

        assert!(human.contains("ordinary Gear: pair"));
        assert!(human.contains("operand `left`"));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["schema"], "conduit.source-sugar-expansion@1");
        assert_eq!(value["expansions"][0]["authored"], "&>");
        assert_eq!(value["expansions"][0]["ordinary_kind"], "pair");
        assert!(
            value["expansions"][0]["source_span"]["line"]
                .as_u64()
                .unwrap()
                > 0
        );
    }

    #[test]
    fn expression_body_reports_its_lossless_ordinary_plot() {
        let path = std::env::temp_dir().join(format!(
            "conduit-expression-body-expansion-{}.conduit",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "plot identity (\n  >> value: Text\n  result: Text >>\n) = .\n",
        )
        .unwrap();

        let human = run(&path, false).unwrap();
        let json = run(&path, true).unwrap();
        std::fs::remove_file(path).unwrap();

        assert!(human.contains("`= .` in plot `identity`"));
        assert!(human.contains("ordinary Gear: conduitese/pure-expression-operation@1"));
        assert!(human.contains("value >> . >> result"));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["expansions"][0]["authored"], "= .");
        assert_eq!(
            value["expansions"][0]["canonical_replacement"],
            "{\n    value >> . >> result\n}"
        );
    }
}
