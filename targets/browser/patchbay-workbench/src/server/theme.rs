//! Finite CSS transport for the shared Patchbay theme.

use conduit_face::{LookFacetAvailability, LookInfo, LookVisualFacetOption, ThemeColor};

pub(crate) fn render_look_css(look: &LookInfo) -> Result<Vec<u8>, LookFacetAvailability> {
    let LookVisualFacetOption::Some(visual) = look.visual() else {
        return Err(LookFacetAvailability::Absent);
    };
    let tokens = visual.tokens();
    let identity = serde_json::to_string(look.identity()).expect("finite Look identity serializes");
    Ok(format!(
        ":root{{--conduit-look-identity:{};--conduit-background:{};--conduit-surface:{};--conduit-structure-primary:{};--conduit-structure-secondary:{};--conduit-text-primary:{};--conduit-text-secondary:{};--conduit-emphasis:{};--conduit-focus:{};--conduit-warning:{};--conduit-failure:{};--conduit-success:{};--conduit-muted:{};--conduit-type-body:{}px;--conduit-type-small:{}px;--conduit-line-height:{}%;--conduit-space-unit:{}px;--conduit-space-control-inline:{}px;--conduit-space-control-block:{}px;--conduit-space-panel:{}px;--conduit-radius-control:{}px;--conduit-radius-panel:{}px;--conduit-focus-width:{}px;--conduit-responsive-breakpoint:{}px;--conduit-responsive-grid-min:{}px;}}\n",
        identity,
        css_color(*tokens.background()),
        css_color(*tokens.surface()),
        css_color(*tokens.structure_primary()),
        css_color(*tokens.structure_secondary()),
        css_color(*tokens.text_primary()),
        css_color(*tokens.text_secondary()),
        css_color(*tokens.emphasis()),
        css_color(*tokens.focus()),
        css_color(*tokens.warning()),
        css_color(*tokens.failure()),
        css_color(*tokens.success()),
        css_color(*tokens.muted()),
        tokens.type_body_px(),
        tokens.type_small_px(),
        tokens.line_height_percent(),
        tokens.space_unit_px(),
        tokens.space_control_inline_px(),
        tokens.space_control_block_px(),
        tokens.space_panel_px(),
        tokens.radius_control_px(),
        tokens.radius_panel_px(),
        tokens.focus_width_px(),
        720, // Structural Mask reflow, not a Look token.
        260, // Structural Mask grid minimum.
    )
    .into_bytes())
}

fn css_color(color: ThemeColor) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        color.red(),
        color.green(),
        color.blue()
    )
}
