use crate::{ThemeColor, PHOSPHOR_THEME};
use conduit_core::CharacteristicId;
use conduit_planner::{
    dos_shell_style, PlannerFactRef, PlannerFactValue, PlannerPreference,
    PRESENTATION_PALETTE_CLASS,
};

#[test]
fn phosphor_theme_is_fixed_bounded_and_matches_the_shared_palette() {
    assert_eq!(PHOSPHOR_THEME.identity, "conduit.patchbay/phosphor@1");
    assert_eq!(PHOSPHOR_THEME.background.packed_rgb(), 0x0005_070B);
    assert_eq!(PHOSPHOR_THEME.surface.packed_rgb(), 0x0009_0D16);
    assert_eq!(PHOSPHOR_THEME.structure_primary.packed_rgb(), 0x000D_D8F6);
    assert_eq!(PHOSPHOR_THEME.structure_secondary.packed_rgb(), 0x000A_1F87);
    assert_eq!(PHOSPHOR_THEME.emphasis.packed_rgb(), 0x00E9_A325);
    assert_eq!(PHOSPHOR_THEME.focus.packed_rgb(), 0x00F4_C400);
    assert!(core::mem::size_of_val(&PHOSPHOR_THEME) <= 64);
}

#[test]
fn ordinary_text_and_focus_have_readable_contrast_without_glow() {
    assert!(contrast(PHOSPHOR_THEME.text_primary, PHOSPHOR_THEME.background) >= 4.5);
    assert!(contrast(PHOSPHOR_THEME.emphasis, PHOSPHOR_THEME.background) >= 4.5);
    assert!(contrast(PHOSPHOR_THEME.focus, PHOSPHOR_THEME.background) >= 7.0);
}

#[test]
fn deuteranopia_simulation_keeps_focus_and_emphasis_distinct_from_the_field() {
    let background = simulate_deuteranopia(PHOSPHOR_THEME.background);
    let focus = simulate_deuteranopia(PHOSPHOR_THEME.focus);
    let emphasis = simulate_deuteranopia(PHOSPHOR_THEME.emphasis);
    assert!(contrast(focus, background) >= 7.0);
    assert!(contrast(emphasis, background) >= 4.5);
    assert!(color_distance(focus, background) >= 180);
}

#[test]
fn phosphor_decoration_is_a_truthful_realization_of_the_dos_shell_palette_preference() {
    let style = dos_shell_style();
    assert_eq!(PHOSPHOR_THEME.identity, "conduit.patchbay/phosphor@1");
    assert!(style.preferences.contains(&PlannerPreference::PreferEqual {
        fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
            PRESENTATION_PALETTE_CLASS,
        )),
        value: PlannerFactValue::Category("phosphor-cyan-amber".into()),
    }));
}

fn contrast(left: ThemeColor, right: ThemeColor) -> f64 {
    let lighter = luminance(left).max(luminance(right));
    let darker = luminance(left).min(luminance(right));
    (lighter + 0.05) / (darker + 0.05)
}

fn luminance(color: ThemeColor) -> f64 {
    0.2126 * linear(color.red()) + 0.7152 * linear(color.green()) + 0.0722 * linear(color.blue())
}

fn linear(channel: u8) -> f64 {
    let channel = f64::from(channel) / 255.0;
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn simulate_deuteranopia(color: ThemeColor) -> ThemeColor {
    let red = 0.625 * f64::from(color.red()) + 0.375 * f64::from(color.green());
    let green = 0.700 * f64::from(color.red()) + 0.300 * f64::from(color.green());
    let blue = 0.300 * f64::from(color.green()) + 0.700 * f64::from(color.blue());
    ThemeColor::from_rgb(red.round() as u8, green.round() as u8, blue.round() as u8)
}

fn color_distance(left: ThemeColor, right: ThemeColor) -> u16 {
    u16::from(left.red().abs_diff(right.red()))
        + u16::from(left.green().abs_diff(right.green()))
        + u16::from(left.blue().abs_diff(right.blue()))
}
