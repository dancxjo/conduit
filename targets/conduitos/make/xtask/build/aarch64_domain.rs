//! Instrument the existing AArch64 product artifact with independent hostile entries.
use super::*;

pub(crate) fn execute(
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
    execute_with_features(
        ConduitosArch::Aarch64,
        opts,
        &["aarch64-product", "ordinary-domain-proof"],
        Some(ProfileMake {
            generated: &generated,
            build_id,
            image_binding,
        }),
        ArtifactRole::ArchitectureProofAppliance,
        ConduitOsProductArtifact::for_target("conduitos/aarch64/virt"),
    )
}
