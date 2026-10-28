//! Exact board-bound ARMv6 image variants sharing the same firmware/FAT builder.
use super::{armv6_rpi_board::Armv6RpiBoard, profile::Paths, ConduitosArch, ConduitosError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RpiImageProfile {
    Product,
    MiniPitft,
}

impl RpiImageProfile {
    pub(super) fn validate(self, board: Armv6RpiBoard) -> Result<(), ConduitosError> {
        if self == Self::MiniPitft && board != Armv6RpiBoard::BPlusV1_2 {
            return Err(ConduitosError::refusal(
                "unsupported-minipitft-board",
                "Mini PiTFT requires Raspberry Pi Model B+ v1.2",
            ));
        }
        Ok(())
    }

    pub(super) fn paths(self) -> Result<Paths, ConduitosError> {
        let mut paths = Paths::new(ConduitosArch::Armv6)?;
        if self == Self::MiniPitft {
            paths.target = paths.target.join("rpi-b-plus-minipitft");
            paths.kernel = paths.target.join(self.binary());
        }
        Ok(paths)
    }

    pub(super) const fn binary(self) -> &'static str {
        match self {
            Self::Product => "conduitos-armv6-rpi-b-plus-a3",
            Self::MiniPitft => "conduitos-armv6-rpi-b-plus-minipitft",
        }
    }

    pub(super) const fn feature(self) -> &'static str {
        match self {
            Self::Product => "armv6-rpi-b-plus-a3",
            Self::MiniPitft => "armv6-rpi-b-plus-minipitft",
        }
    }

    pub(super) fn artifact_slug(self, board: Armv6RpiBoard) -> String {
        match self {
            Self::Product => board.artifact_slug().to_owned(),
            Self::MiniPitft => "rpi-b-plus-minipitft".to_owned(),
        }
    }

    pub(super) fn identity_slug(self, board: Armv6RpiBoard) -> String {
        match self {
            Self::Product => board.identity_slug().to_owned(),
            Self::MiniPitft => "armv6-rpi-b-plus-minipitft".to_owned(),
        }
    }

    pub(super) const fn config(self) -> &'static str {
        match self {
            Self::Product => "",
            // SPI0 is derived from the VideoCore core clock. Fix that clock
            // across idle/load without requiring force_turbo or overclocking.
            Self::MiniPitft => {
                "# Adafruit Mini PiTFT 1.3 inch, B+ v1.2 only\ncore_freq=250\ncore_freq_min=250\n"
            }
        }
    }

    pub(super) fn boot_config(self, heading: &str, common: &str) -> String {
        let common = if self == Self::MiniPitft {
            // gpu_mem=16 selects start_cd.elf/fixup_cd.dat. This profile
            // packages the pinned standard pair start.elf/fixup.dat instead.
            // https://www.raspberrypi.com/documentation/computers/config_txt.html
            common.replace("gpu_mem=16\n", "gpu_mem=32\n")
        } else {
            common.to_owned()
        };
        format!("{heading}{common}{}", self.config())
    }
}

pub(super) fn execute_minipitft(opts: &crate::cli::GlobalOpts) -> Result<(), ConduitosError> {
    super::make_resolution::require_make_target(
        ConduitosArch::Armv6,
        Some(Armv6RpiBoard::BPlusV1_2),
    )?;
    super::armv6_rpi_b_plus_image::execute_profile(
        Armv6RpiBoard::BPlusV1_2,
        RpiImageProfile::MiniPitft,
        opts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minipitft_command_is_explicit_and_accepts_no_other_board() {
        use crate::cli::Cli;
        use clap::Parser;
        assert!(Cli::try_parse_from([
            "xtask",
            "make",
            "conduitos",
            "rpi-minipitft-image",
            "--dry-run"
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "xtask",
            "make",
            "conduitos",
            "rpi-minipitft-image",
            "--board",
            "rpi-zero-v1"
        ])
        .is_err());
    }

    #[test]
    fn minipitft_is_b_plus_only_with_fixed_non_turbo_core_clock() {
        let profile = RpiImageProfile::MiniPitft;
        assert!(profile.validate(Armv6RpiBoard::BPlusV1_2).is_ok());
        for board in [
            Armv6RpiBoard::ZeroV1,
            Armv6RpiBoard::ZeroWV1_1,
            Armv6RpiBoard::ZeroWhV1_1,
        ] {
            assert!(profile.validate(board).is_err());
        }
        assert!(profile.config().contains("core_freq=250\n"));
        assert!(profile.config().contains("core_freq_min=250\n"));
        assert!(!profile.config().contains("force_turbo="));
        assert_eq!(RpiImageProfile::Product.config(), "");
        assert_ne!(profile.binary(), RpiImageProfile::Product.binary());
    }
}
