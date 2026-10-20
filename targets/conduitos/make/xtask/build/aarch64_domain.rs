//! Instrument the existing AArch64 product artifact with independent hostile entries.
use super::*;

pub(crate) fn execute(
    manifest: &BuildManifest,
    profile_id: &str,
    build_id: &str,
    image_binding: &str,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let generated = paths.target.join("make-record.rs");
    let source = fs::read_to_string(&generated)
        .map_err(|error| ConduitosError::refusal("proof-make-unavailable", error.to_string()))?;
    for identity in [profile_id, build_id, image_binding] {
        if !source.contains(&format!("{identity:?}")) {
            return Err(ConduitosError::refusal("proof-make-mismatch", identity));
        }
    }
    let mut features = target_lowering::lower(manifest)?.cargo_features;
    features.push("ordinary-domain-proof");
    execute_with_features(
        ConduitosArch::Aarch64,
        opts,
        &features,
        Some(ProfileMake {
            generated: &generated,
            build_id,
            image_binding,
        }),
        ArtifactRole::ArchitectureProofAppliance,
        ConduitOsProductArtifact::for_target("conduitos/aarch64/virt"),
    )
}
