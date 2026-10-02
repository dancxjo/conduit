use crate::cli::DiagramFormat;
use std::path::Path;

pub(crate) fn run(form: &Path, format: DiagramFormat, output: Option<&Path>) -> Result<(), String> {
    let source = crate::form_source::load(form)?;
    let authoring = source.expand_entry_for_authoring()?;
    let information_labels = patchbay_svg_mask::information_labels(&authoring, &source.startup);
    let rendered = match format {
        DiagramFormat::Svg => patchbay_svg_mask::render_svg(&authoring, &information_labels),
        DiagramFormat::Mermaid => {
            patchbay_svg_mask::render_mermaid(&authoring, &information_labels)
        }
    };
    if let Some(path) = output {
        std::fs::write(path, rendered).map_err(|error| error.to_string())
    } else {
        print!("{rendered}");
        Ok(())
    }
}
