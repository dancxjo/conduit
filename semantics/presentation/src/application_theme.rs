//! Renderer-neutral application theme roles and their finite transport.

use crate::{ApplicationViewRefusal, APPLICATION_VIEW_VERSION};
use alloc::vec::Vec;

pub const MAX_APPLICATION_THEME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThemeColor(pub u8, pub u8, pub u8);

impl ThemeColor {
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(red, green, blue)
    }
    pub const fn red(self) -> u8 {
        self.0
    }
    pub const fn green(self) -> u8 {
        self.1
    }
    pub const fn blue(self) -> u8 {
        self.2
    }
    pub const fn packed_rgb(self) -> u32 {
        ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | self.2 as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationTheme {
    pub identity: &'static str,
    pub background: ThemeColor,
    pub reading_paper: ThemeColor,
    pub workbench_canvas: ThemeColor,
    pub bootstrap_surface: ThemeColor,
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

pub const CONDUIT_APPLICATION_THEME: ApplicationTheme = ApplicationTheme {
    identity: "conduit.presentation/phosphor@1",
    background: ThemeColor(0x05, 0x07, 0x0b),
    reading_paper: ThemeColor(0x0c, 0x12, 0x1c),
    workbench_canvas: ThemeColor(0x05, 0x07, 0x0b),
    bootstrap_surface: ThemeColor(0x09, 0x0d, 0x16),
    surface: ThemeColor(0x09, 0x0d, 0x16),
    structure_primary: ThemeColor(0x0d, 0xd8, 0xf6),
    structure_secondary: ThemeColor(0x0a, 0x1f, 0x87),
    text_primary: ThemeColor(0x93, 0xd2, 0xf7),
    text_secondary: ThemeColor(0x57, 0x8e, 0xc9),
    emphasis: ThemeColor(0xe9, 0xa3, 0x25),
    focus: ThemeColor(0xf4, 0xc4, 0x00),
    warning: ThemeColor(0xf2, 0xbd, 0x71),
    failure: ThemeColor(0xff, 0x72, 0x72),
    success: ThemeColor(0x63, 0xd6, 0x9b),
    muted: ThemeColor(0x8c, 0x4c, 0x19),
};

impl ApplicationTheme {
    pub fn encode(&self) -> Result<Vec<u8>, ApplicationViewRefusal> {
        if self.identity.is_empty() || self.identity.len() > 64 {
            return Err(ApplicationViewRefusal::TextTooLong);
        }
        let colors = [
            self.background,
            self.reading_paper,
            self.workbench_canvas,
            self.bootstrap_surface,
            self.surface,
            self.structure_primary,
            self.structure_secondary,
            self.text_primary,
            self.text_secondary,
            self.emphasis,
            self.focus,
            self.warning,
            self.failure,
            self.success,
            self.muted,
        ];
        let mut encoded = Vec::with_capacity(2 + self.identity.len() + colors.len() * 3);
        encoded.push(APPLICATION_VIEW_VERSION);
        encoded.push(self.identity.len() as u8);
        encoded.extend_from_slice(self.identity.as_bytes());
        for ThemeColor(red, green, blue) in colors {
            encoded.extend_from_slice(&[red, green, blue]);
        }
        if encoded.len() > MAX_APPLICATION_THEME_BYTES {
            return Err(ApplicationViewRefusal::OversizedEncoding);
        }
        Ok(encoded)
    }
}
