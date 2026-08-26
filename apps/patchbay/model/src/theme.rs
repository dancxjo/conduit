//! Toolkit-independent, renderer-facing Patchbay theme tokens.
//!
//! Theme values are presentation decoration. They are deliberately absent
//! from Form, Body, Wake, Plan, Play, Host, Line, Sign, and renderer-plan
//! identities.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThemeColor {
    red: u8,
    green: u8,
    blue: u8,
}

impl ThemeColor {
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub const fn red(self) -> u8 {
        self.red
    }

    pub const fn green(self) -> u8 {
        self.green
    }

    pub const fn blue(self) -> u8 {
        self.blue
    }

    pub const fn packed_rgb(self) -> u32 {
        ((self.red as u32) << 16) | ((self.green as u32) << 8) | self.blue as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchbayTheme {
    pub identity: &'static str,
    pub background: ThemeColor,
    pub surface: ThemeColor,
    pub structure_primary: ThemeColor,
    pub structure_secondary: ThemeColor,
    pub text_primary: ThemeColor,
    pub text_secondary: ThemeColor,
    pub emphasis: ThemeColor,
    pub focus: ThemeColor,
    pub warning: ThemeColor,
    pub failure: ThemeColor,
    pub success: ThemeColor,
    pub muted: ThemeColor,
}

pub const PHOSPHOR_THEME: PatchbayTheme = PatchbayTheme {
    identity: "conduit.patchbay/phosphor@1",
    background: ThemeColor::from_rgb(0x05, 0x07, 0x0B),
    surface: ThemeColor::from_rgb(0x09, 0x0D, 0x16),
    structure_primary: ThemeColor::from_rgb(0x0D, 0xD8, 0xF6),
    structure_secondary: ThemeColor::from_rgb(0x0A, 0x1F, 0x87),
    text_primary: ThemeColor::from_rgb(0x93, 0xD2, 0xF7),
    text_secondary: ThemeColor::from_rgb(0x57, 0x8E, 0xC9),
    emphasis: ThemeColor::from_rgb(0xE9, 0xA3, 0x25),
    focus: ThemeColor::from_rgb(0xF4, 0xC4, 0x00),
    warning: ThemeColor::from_rgb(0xF2, 0xBD, 0x71),
    failure: ThemeColor::from_rgb(0xFF, 0x72, 0x72),
    success: ThemeColor::from_rgb(0x63, 0xD6, 0x9B),
    muted: ThemeColor::from_rgb(0x8C, 0x4C, 0x19),
};
