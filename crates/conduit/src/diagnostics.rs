use conduit_form::{parse_document, structured_form_diagnostic};
use std::fs;
use std::path::Path;

pub(crate) fn run(path: &Path, json: bool) -> Result<bool, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let catalog = conduit_std_catalog::standard_profile_catalog();
    let document = parse_document(&source, &catalog);
    let diagnostics = document
        .diagnostics
        .iter()
        .map(|diagnostic| structured_form_diagnostic(&source, diagnostic))
        .collect::<Vec<_>>();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&diagnostics).map_err(|error| error.to_string())?
        );
    } else if diagnostics.is_empty() {
        println!("No form diagnostics.");
    } else {
        for diagnostic in &diagnostics {
            println!("{}", diagnostic.render_human());
        }
    }
    Ok(diagnostics.is_empty())
}
