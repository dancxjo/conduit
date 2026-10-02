//! Product builds check their selected target; architecture appliances retain
//! the shared cross-target backbone gate.
use super::super::{
    armv6_rpi_b_plus_a0,
    profile::{Paths, COMMON_BACKBONE_TARGETS},
    ConduitosError,
};
use crate::cli::GlobalOpts;
use std::process::Command;

fn targets<'a>(product_target: &'a Option<&'static str>) -> &'a [&'static str] {
    if product_target.is_some() {
        product_target.as_slice()
    } else {
        COMMON_BACKBONE_TARGETS
    }
}

pub(super) fn check(
    paths: &Paths,
    opts: &GlobalOpts,
    product_target: Option<&'static str>,
) -> Result<(), ConduitosError> {
    for target in targets(&product_target) {
        let mut command = Command::new("cargo");
        command.arg("check");
        if *target == armv6_rpi_b_plus_a0::TARGET {
            command
                .arg("-Zbuild-std=core,alloc")
                .env("RUSTC_BOOTSTRAP", "1");
        }
        command
            .args(["-p", "conduitos", "--lib", "--target", target])
            .current_dir(&paths.root);
        if opts.locked {
            command.arg("--locked");
        }
        let status = command.status().map_err(|error| {
            ConduitosError::refusal(
                "matrix-toolchain-unavailable",
                format!("cannot check common backbone for {target}: {error}"),
            )
        })?;
        if !status.success() {
            return Err(ConduitosError::refusal(
                "matrix-common-backbone-failed",
                format!("shared ConduitOS backbone did not compile for {target}"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_conduitos_make::ConduitOsProductArtifact;

    #[test]
    fn product_preflight_needs_only_the_selected_compiler_target() {
        for target in [
            "conduitos/x86_64/pc",
            "conduitos/ia32/pc",
            "conduitos/aarch64/virt",
            "conduitos/riscv64/virt",
            "conduitos/loongarch64/virt",
        ] {
            let artifact = ConduitOsProductArtifact::for_target(target).unwrap();
            assert_eq!(targets(&Some(artifact.rust_target)), [artifact.rust_target]);
        }
    }

    #[test]
    fn architecture_appliance_retains_all_common_backbone_targets() {
        assert_eq!(targets(&None), COMMON_BACKBONE_TARGETS);
    }
}
