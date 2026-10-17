//! Public Source formatting without execution or implicit file writes.
use std::path::Path;

pub(crate) fn run(path: &Path, check: bool) -> Result<String, String> {
    let source = crate::plot_source::load(path)?;
    let formatted = conduit_plot::format_syntax(&source.source, &source.startup)
        .map_err(|error| format!("cannot format Source: {error:?}"))?;
    if check {
        if formatted != source.source {
            return Err(format!("{} needs formatting", path.display()));
        }
        Ok(String::new())
    } else {
        Ok(formatted)
    }
}
