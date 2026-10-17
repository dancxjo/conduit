//! Checked shipment of the portable pattern family's Source-owned glyph notation metadata.
use alloc::{format, string::String};
use conduit_plot::{
    CheckedPackageBundle, PackageExportCatalog, PackageMemberSource, ProfileCatalog, StartupCatalog,
};

pub const NOTATION_EXPORT_PATH: &str = "text/pattern/notation";
const MANIFEST: &str = include_str!("../pattern/pack.conduit");
const NOTATION: &str = include_str!("../pattern/notation.conduit");

pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<(), String> {
    let manifest = conduit_plot::parse_syntax_document(MANIFEST);
    let package = manifest
        .packages
        .first()
        .ok_or("invalid Text pattern notation pack manifest")?;
    let members = [PackageMemberSource {
        path: "notation",
        source: NOTATION,
    }];
    let bundle = CheckedPackageBundle::from_sources(MANIFEST, package, &members)
        .map_err(|e| format!("{e:?}"))?;
    let exports = PackageExportCatalog::from_bundle(&bundle, MANIFEST, package, &members)
        .map_err(|e| format!("{e:?}"))?;
    if exports
        .resolve_glyph_notation(NOTATION_EXPORT_PATH)
        .is_none()
    {
        return Err(
            "Text pattern notation path differs from its actual shipped declaration".into(),
        );
    }
    exports
        .install_shipped_glyph_notations(startup, profile)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}
