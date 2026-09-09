//! Fixed native graphical resources, below portable Presentation meaning.
//! No selectors, inheritance, external configuration, or dynamic allocation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Retained surfaces use canonical RGB before scanout format conversion.
    pub const fn packed(self) -> u32 {
        ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | self.2 as u32
    }
}

pub struct NativeStyle {
    pub inset: u16,
    pub panel_inset: u16,
    pub gap: u16,
    pub border_width: u16,
    pub corner_radius: u16,
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    pub muted: Rgb,
    pub success: Rgb,
    pub warning: Rgb,
    pub danger: Rgb,
    pub focus: Rgb,
    pub hovered: Rgb,
    pub selected: Rgb,
}

pub const NATIVE_STYLE: NativeStyle = NativeStyle {
    inset: 8,
    panel_inset: 12,
    gap: 12,
    border_width: 1,
    corner_radius: 6,
    background: Rgb(15, 23, 32),
    foreground: Rgb(225, 232, 240),
    accent: Rgb(83, 178, 255),
    muted: Rgb(154, 170, 186),
    success: Rgb(75, 218, 122),
    warning: Rgb(255, 190, 70),
    danger: Rgb(255, 107, 122),
    focus: Rgb(255, 204, 51),
    hovered: Rgb(153, 210, 255),
    selected: Rgb(83, 178, 255),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_and_geometry_are_fixed_small_resources() {
        assert!(core::mem::size_of::<NativeStyle>() <= 64);
        assert!(NATIVE_STYLE.border_width <= NATIVE_STYLE.inset);
        assert!(NATIVE_STYLE.corner_radius <= NATIVE_STYLE.inset);
        assert_eq!(NATIVE_STYLE.accent.packed(), 0x53b2ff);
        assert_eq!(NATIVE_STYLE.focus.packed(), 0xffcc33);
    }
}
