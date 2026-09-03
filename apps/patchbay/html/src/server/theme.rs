//! Finite CSS transport for the shared Patchbay theme.

use patchbay_model::{ApplicationTheme, ThemeColor};

pub(super) fn render_theme_css(theme: &ApplicationTheme) -> Vec<u8> {
    format!(
        ":root{{--conduit-theme-identity:\"{}\";--conduit-background:{};--conduit-surface:{};--conduit-structure-primary:{};--conduit-structure-secondary:{};--conduit-text-primary:{};--conduit-text-secondary:{};--conduit-emphasis:{};--conduit-focus:{};--conduit-warning:{};--conduit-failure:{};--conduit-success:{};--conduit-muted:{};--conduit-type-body:{}px;--conduit-type-small:{}px;--conduit-line-height:{}%;--conduit-space-unit:{}px;--conduit-space-control-inline:{}px;--conduit-space-control-block:{}px;--conduit-space-panel:{}px;--conduit-radius-control:{}px;--conduit-radius-panel:{}px;--conduit-focus-width:{}px;--conduit-responsive-breakpoint:{}px;--conduit-responsive-grid-min:{}px;}}\n",
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
        theme.type_body_px,
        theme.type_small_px,
        theme.line_height_percent,
        theme.space_unit_px,
        theme.space_control_inline_px,
        theme.space_control_block_px,
        theme.space_panel_px,
        theme.radius_control_px,
        theme.radius_panel_px,
        theme.focus_width_px,
        theme.responsive_breakpoint_px,
        theme.responsive_grid_min_px,
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
