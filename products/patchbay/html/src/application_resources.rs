//! Delivery from the build-checked application registry. Generated inputs are explicit.
//!
//! Edit `assets/patchbay.application.template.json` to add or remove an admitted
//! resource. `source` is a repository-relative file, `generated:theme`, or
//! `supplied:runtime`; `kind` determines the HTTP media type. The build embeds
//! file sources and checks their bounds. Manifest sealing checks all inputs.
//! No independent delivery inventory or runtime filesystem lookup is required.
use std::path::Path;

pub(crate) struct Resource {
    path: &'static str,
    media_type: &'static str,
    source: Source,
}
enum Source {
    Embedded(&'static [u8]),
    Theme,
    Runtime,
}
include!(concat!(env!("OUT_DIR"), "/resources.rs"));

pub(crate) fn resource<'a>(
    path: &str,
    runtime: &'a [u8],
    theme: &'a [u8],
) -> Option<(&'static str, &'a [u8])> {
    let resource = RESOURCES.iter().find(|resource| resource.path == path)?;
    let bytes = match resource.source {
        Source::Embedded(bytes) => bytes,
        Source::Theme => theme,
        Source::Runtime => runtime,
    };
    Some((resource.media_type, bytes))
}

/// Stage the same admitted bytes served by the embedded product. The manifest
/// validates all dynamic bounds before any package resource is written.
pub fn stage(destination: &Path, runtime: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let theme = crate::application_theme_css();
    let manifest = conduit_browser_host::application_package::build_manifest(
        include_bytes!("../assets/patchbay.application.template.json"),
        |path| resource(path, runtime, &theme).map(|(_, bytes)| bytes),
    )?;
    std::fs::create_dir_all(destination.join("assets"))?;
    for entry in RESOURCES {
        let (_, bytes) = resource(entry.path, runtime, &theme).unwrap();
        std::fs::write(destination.join(entry.path), bytes)?;
    }
    std::fs::write(destination.join("patchbay.application.json"), manifest)?;
    Ok(())
}
