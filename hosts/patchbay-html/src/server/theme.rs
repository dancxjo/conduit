//! Finite CSS transport for the shared Patchbay theme.

use patchbay_model::{PatchbayTheme, ThemeColor};

pub(super) fn render_theme_css(theme: &PatchbayTheme) -> Vec<u8> {
    format!(
        ":root{{--patchbay-theme-identity:\"{}\";--patchbay-background:{};--patchbay-surface:{};--patchbay-structure-primary:{};--patchbay-structure-secondary:{};--patchbay-text-primary:{};--patchbay-text-secondary:{};--patchbay-emphasis:{};--patchbay-focus:{};--patchbay-warning:{};--patchbay-failure:{};--patchbay-success:{};--patchbay-muted:{};}}\n",
        theme.identity,
        css_color(theme.background),
        css_color(theme.surface),
        css_color(theme.structure_primary),
        css_color(theme.structure_secondary),
        css_color(theme.text_primary),
        css_color(theme.text_secondary),
        css_color(theme.emphasis),
        css_color(theme.focus),
        css_color(theme.warning),
        css_color(theme.failure),
        css_color(theme.success),
        css_color(theme.muted),
    )
    .into_bytes()
}

fn css_color(color: ThemeColor) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        color.red(),
        color.green(),
        color.blue()
    )
}
