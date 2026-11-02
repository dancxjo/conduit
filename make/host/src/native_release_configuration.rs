//! Bind authored configuration to an already-admitted native implementation closure.
//! This manufactures IMAGE metadata, never executable bytes or toolchain evidence.
use crate::{
    build_host_image, BuildInputs, CheckedHostConfiguration, HostImage, MakeCatalog,
    MakePackageSet, SporeOutputKind, VerifiedHostReleaseBytes,
};

/// A prior native release may be configured without recompiling only inside the
/// exact reviewed Base/feature/package/toolchain closure. The caller separately
/// owns actual payload custody, finite workspace and current review authority.
/// Browser bundles require their target-specific rich IMAGE proof instead.
pub fn bind_native_release_configuration(
    admitted: &CheckedHostConfiguration,
    admitted_image: &HostImage,
    selected: &CheckedHostConfiguration,
    release: &VerifiedHostReleaseBytes,
    catalog: &MakeCatalog,
    packages: &MakePackageSet,
) -> Result<(HostImage, Vec<u8>), String> {
    let baseline = admitted.profile();
    let profile = selected.profile();
    let output = SporeOutputKind::NativeBundle;
    let m = release.manifest();
    if m.output != "native-bundle"
        || m.source_identity.is_empty()
        || admitted_image.manifest.source_identity != m.source_identity
        || admitted_image.manifest.source_configuration_id.as_deref()
            != Some(admitted.configuration_id())
        || baseline.target != profile.target
        || profile.target.key() != m.target_id
        || baseline.policy != profile.policy
        || baseline.host_core != profile.host_core
        || baseline.fragments != profile.fragments
        || baseline.bases != profile.bases
        || baseline.drivers != profile.drivers
        || baseline.lines != profile.lines
        || baseline.presenters != profile.presenters
        || baseline.facilities != profile.facilities
        || baseline.exclusions != profile.exclusions
    {
        return Err("authored native Host exceeds admitted target/implementation policy".into());
    }
    let original = packages
        .derive_build_selection(baseline, &output)
        .map_err(|e| format!("admitted native Make closure: {e:?}"))?;
    let chosen = packages
        .derive_build_selection(profile, &output)
        .map_err(|e| format!("authored native Make closure: {e:?}"))?;
    if original != chosen
        || m.make_package_id != chosen.make_package_id
        || m.builder_adapter != chosen.builder_adapter
        || Some(m.deployment_adapter.as_str()) != chosen.deployment_adapter.as_deref()
    {
        return Err("authored native Host differs from retained release Make closure".into());
    }
    // The existing IMAGE resolver's boolean gates metadata resolution. No new
    // compiler availability or source-build receipt is asserted: executable
    // authority comes exclusively from the admitted opaque prior-release proof.
    let inputs = BuildInputs {
        source_identity: m.source_identity.clone(),
        toolchain_available: true,
    };
    let (expected, _) = build_host_image(baseline.clone(), catalog, packages, &output, &inputs)
        .map_err(|e| format!("admitted native IMAGE: {e:?}"))?;
    if &expected != admitted_image {
        return Err("admitted native IMAGE differs from its checked configuration/source".into());
    }
    build_host_image(profile.clone(), catalog, packages, &output, &inputs)
        .map_err(|e| format!("authored native IMAGE: {e:?}"))
}
