use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum Armv6RpiBoard {
    #[value(name = "rpi-b-plus-v1.2")]
    #[default]
    BPlusV1_2,
    #[value(name = "rpi-zero-v1")]
    ZeroV1,
    #[value(name = "rpi-zero-w-v1.1")]
    ZeroWV1_1,
    #[value(name = "rpi-zero-wh-v1.1")]
    ZeroWhV1_1,
}

impl Armv6RpiBoard {
    pub const fn id(self) -> &'static str {
        match self {
            Self::BPlusV1_2 => "raspberry-pi-model-b-plus-v1.2",
            Self::ZeroV1 => "raspberry-pi-zero-v1",
            Self::ZeroWV1_1 => "raspberry-pi-zero-w-v1.1",
            Self::ZeroWhV1_1 => "raspberry-pi-zero-wh-v1.1",
        }
    }

    pub const fn identity_slug(self) -> &'static str {
        match self {
            Self::BPlusV1_2 => "armv6-rpi-b-plus",
            Self::ZeroV1 => "armv6-rpi-zero-v1",
            Self::ZeroWV1_1 => "armv6-rpi-zero-w-v1.1",
            Self::ZeroWhV1_1 => "armv6-rpi-zero-wh-v1.1",
        }
    }

    pub const fn artifact_slug(self) -> &'static str {
        match self {
            Self::BPlusV1_2 => "rpi-b-plus",
            Self::ZeroV1 => "rpi-zero-v1",
            Self::ZeroWV1_1 => "rpi-zero-w-v1.1",
            Self::ZeroWhV1_1 => "rpi-zero-wh-v1.1",
        }
    }

    pub const fn config_heading(self) -> &'static str {
        match self {
            Self::BPlusV1_2 => "# ConduitOS Raspberry Pi Model B+ v1.2\n",
            Self::ZeroV1 => "# ConduitOS original Raspberry Pi Zero v1\n",
            Self::ZeroWV1_1 => "# ConduitOS Raspberry Pi Zero W v1.1\n",
            Self::ZeroWhV1_1 => "# ConduitOS Raspberry Pi Zero WH v1.1\n",
        }
    }

    pub const fn accepts_revision(self, revision: u32) -> bool {
        match self {
            Self::BPlusV1_2 => matches!(revision, 0x000010 | 0x000013 | 0x900032),
            Self::ZeroV1 => matches!(revision, 0x900092 | 0x920092 | 0x900093 | 0x920093),
            Self::ZeroWV1_1 | Self::ZeroWhV1_1 => matches!(revision, 0x9000c1 | 0x9200c1),
        }
    }
}
