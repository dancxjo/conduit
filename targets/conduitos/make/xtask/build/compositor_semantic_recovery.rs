//! Same admitted native graphical Make, with an explicit different proof entry.
use super::*;
pub(in super::super) fn execute(
    opts: &GlobalOpts,
) -> Result<(BuildRecord, BuildManifest), ConduitosError> {
    let manifest = resolve_embedded_profile(
        ConduitosArch::X86_64,
        include_str!("../../../profiles/conduitos-native.host.conduit"),
    )?;
    let build = execute_profile_with_features(
        &manifest,
        ArtifactRole::ArchitectureProofAppliance,
        opts,
        &["compositor-semantic-recovery-proof"],
    )?;
    Ok((build, manifest))
}
