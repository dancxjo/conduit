use super::{Armv6RpiBoard, ConduitosArch, ConduitosError};

pub(super) fn require_fabrication_target(
    arch: ConduitosArch,
    board: Option<Armv6RpiBoard>,
) -> Result<(), ConduitosError> {
    let packages = conduit_workspace_fabrication::package_set();
    let candidates = packages
        .target_descriptors()
        .into_iter()
        .filter(|target| {
            target.family == "conduitos"
                && target.architecture == arch.as_str()
                && (arch != ConduitosArch::Armv6
                    || target.board.as_deref() == Some(board.unwrap_or_default().id()))
        })
        .collect::<Vec<_>>();
    let [target] = candidates.as_slice() else {
        return Err(ConduitosError::refusal(
            "fabrication-target-resolution-failed",
            format!(
                "{} resolved to {} package targets",
                arch.as_str(),
                candidates.len()
            ),
        ));
    };
    let anchor = packages
        .anchor_for_target(&target.key())
        .expect("resolved descriptor has one checked anchor");
    let expected_package = if arch == ConduitosArch::Armv6 {
        "conduit-host-raspberry-pi@1"
    } else {
        "conduitos-image@1"
    };
    if anchor.package_id != expected_package {
        return Err(ConduitosError::refusal(
            "fabrication-package-mismatch",
            format!("{} is owned by {}", target.key(), anchor.package_id),
        ));
    }
    Ok(())
}

pub(super) fn reject_board_for_non_armv6(
    arch: ConduitosArch,
    board: Option<Armv6RpiBoard>,
) -> Result<(), ConduitosError> {
    if board.is_some() && arch != ConduitosArch::Armv6 {
        return Err(ConduitosError::refusal(
            "board-architecture-mismatch",
            format!("--board is not valid for {}", arch.as_str()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_buildable_target_resolves_through_its_family_package() {
        for arch in [
            ConduitosArch::Ia32,
            ConduitosArch::X86_64,
            ConduitosArch::Aarch64,
            ConduitosArch::Riscv64,
            ConduitosArch::Loongarch64,
        ] {
            require_fabrication_target(arch, None).unwrap();
        }
        require_fabrication_target(ConduitosArch::Armv6, Some(Armv6RpiBoard::BPlusV1_2)).unwrap();
        require_fabrication_target(ConduitosArch::Armv6, Some(Armv6RpiBoard::ZeroV1)).unwrap();
    }
}
