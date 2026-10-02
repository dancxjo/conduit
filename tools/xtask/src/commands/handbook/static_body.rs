//! Attach the same admitted static application to documentary chapter routes.
use std::{fs, path::Path};

pub(crate) fn attach(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("html") {
            continue;
        }
        let html = fs::read_to_string(&path)?;
        let html = html.replace(
            "</head>",
            "<meta name=\"conduit-application-package\" content=\"application.application.json\"></head>",
        ).replace(
            "</body>",
            "<script type=\"module\" src=\"host/assets/browser-application-loader.mjs\"></script></body>",
        );
        fs::write(path, html)?;
    }
    Ok(())
}
