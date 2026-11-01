//! A fixed typed value record during preparation; no selectors or cascade.
use super::LocalLookRefusal;
use crate::*;
pub(super) struct Proposal {
    background: ThemeColor,
    bootstrap_surface: ThemeColor,
    density: VisualDensityPreference,
    emphasis: ThemeColor,
    failure: ThemeColor,
    focus: ThemeColor,
    focus_width_px: u16,
    line_height_percent: u16,
    motion: VisualMotionPreference,
    muted: ThemeColor,
    radius_control_px: u16,
    radius_panel_px: u16,
    reading_paper: ThemeColor,
    space_control_block_px: u16,
    space_control_inline_px: u16,
    space_panel_px: u16,
    space_unit_px: u16,
    structure_primary: ThemeColor,
    structure_secondary: ThemeColor,
    success: ThemeColor,
    surface: ThemeColor,
    text_primary: ThemeColor,
    text_scale_percent: u16,
    text_secondary: ThemeColor,
    type_body_px: u16,
    type_small_px: u16,
    typography: VisualTypographyPreference,
    warning: ThemeColor,
    workbench_canvas: ThemeColor,
}
impl Proposal {
    pub fn new(visual: &crate::generated::LookVisualFacetOptionSome) -> Self {
        let tokens = visual.tokens();
        let expression = visual.expression();
        Self {
            background: *tokens.background(),
            bootstrap_surface: *tokens.bootstrap_surface(),
            density: *expression.density(),
            emphasis: *tokens.emphasis(),
            failure: *tokens.failure(),
            focus: *tokens.focus(),
            focus_width_px: *tokens.focus_width_px(),
            line_height_percent: *tokens.line_height_percent(),
            motion: *expression.motion(),
            muted: *tokens.muted(),
            radius_control_px: *tokens.radius_control_px(),
            radius_panel_px: *tokens.radius_panel_px(),
            reading_paper: *tokens.reading_paper(),
            space_control_block_px: *tokens.space_control_block_px(),
            space_control_inline_px: *tokens.space_control_inline_px(),
            space_panel_px: *tokens.space_panel_px(),
            space_unit_px: *tokens.space_unit_px(),
            structure_primary: *tokens.structure_primary(),
            structure_secondary: *tokens.structure_secondary(),
            success: *tokens.success(),
            surface: *tokens.surface(),
            text_primary: *tokens.text_primary(),
            text_scale_percent: *expression.text_scale_percent(),
            text_secondary: *tokens.text_secondary(),
            type_body_px: *tokens.type_body_px(),
            type_small_px: *tokens.type_small_px(),
            typography: *expression.typography(),
            warning: *tokens.warning(),
            workbench_canvas: *tokens.workbench_canvas(),
        }
    }
    pub fn apply(
        &mut self,
        values: &[LocalLookVisualOverride],
    ) -> Result<alloc::vec::Vec<&'static str>, LocalLookRefusal> {
        let mut seen = 0u32;
        let mut names = alloc::vec::Vec::with_capacity(29);
        for value in values {
            let (index, name) = match value {
                LocalLookVisualOverride::Background(v) => {
                    self.background = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (0, "background")
                }
                LocalLookVisualOverride::BootstrapSurface(v) => {
                    self.bootstrap_surface = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (1, "bootstrap_surface")
                }
                LocalLookVisualOverride::Density(v) => {
                    self.density = *v;
                    (2, "density")
                }
                LocalLookVisualOverride::Emphasis(v) => {
                    self.emphasis = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (3, "emphasis")
                }
                LocalLookVisualOverride::Failure(v) => {
                    self.failure = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (4, "failure")
                }
                LocalLookVisualOverride::Focus(v) => {
                    self.focus = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (5, "focus")
                }
                LocalLookVisualOverride::FocusWidthPx(v) => {
                    self.focus_width_px = *v;
                    (6, "focus_width_px")
                }
                LocalLookVisualOverride::LineHeightPercent(v) => {
                    self.line_height_percent = *v;
                    (7, "line_height_percent")
                }
                LocalLookVisualOverride::Motion(v) => {
                    self.motion = *v;
                    (8, "motion")
                }
                LocalLookVisualOverride::Muted(v) => {
                    self.muted = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (9, "muted")
                }
                LocalLookVisualOverride::RadiusControlPx(v) => {
                    self.radius_control_px = *v;
                    (10, "radius_control_px")
                }
                LocalLookVisualOverride::RadiusPanelPx(v) => {
                    self.radius_panel_px = *v;
                    (11, "radius_panel_px")
                }
                LocalLookVisualOverride::ReadingPaper(v) => {
                    self.reading_paper = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (12, "reading_paper")
                }
                LocalLookVisualOverride::SpaceControlBlockPx(v) => {
                    self.space_control_block_px = *v;
                    (13, "space_control_block_px")
                }
                LocalLookVisualOverride::SpaceControlInlinePx(v) => {
                    self.space_control_inline_px = *v;
                    (14, "space_control_inline_px")
                }
                LocalLookVisualOverride::SpacePanelPx(v) => {
                    self.space_panel_px = *v;
                    (15, "space_panel_px")
                }
                LocalLookVisualOverride::SpaceUnitPx(v) => {
                    self.space_unit_px = *v;
                    (16, "space_unit_px")
                }
                LocalLookVisualOverride::StructurePrimary(v) => {
                    self.structure_primary = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (17, "structure_primary")
                }
                LocalLookVisualOverride::StructureSecondary(v) => {
                    self.structure_secondary =
                        ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (18, "structure_secondary")
                }
                LocalLookVisualOverride::Success(v) => {
                    self.success = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (19, "success")
                }
                LocalLookVisualOverride::Surface(v) => {
                    self.surface = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (20, "surface")
                }
                LocalLookVisualOverride::TextPrimary(v) => {
                    self.text_primary = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (21, "text_primary")
                }
                LocalLookVisualOverride::TextScalePercent(v) => {
                    self.text_scale_percent = *v;
                    (22, "text_scale_percent")
                }
                LocalLookVisualOverride::TextSecondary(v) => {
                    self.text_secondary = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (23, "text_secondary")
                }
                LocalLookVisualOverride::TypeBodyPx(v) => {
                    self.type_body_px = *v;
                    (24, "type_body_px")
                }
                LocalLookVisualOverride::TypeSmallPx(v) => {
                    self.type_small_px = *v;
                    (25, "type_small_px")
                }
                LocalLookVisualOverride::Typography(v) => {
                    self.typography = *v;
                    (26, "typography")
                }
                LocalLookVisualOverride::Warning(v) => {
                    self.warning = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (27, "warning")
                }
                LocalLookVisualOverride::WorkbenchCanvas(v) => {
                    self.workbench_canvas = ThemeColor::from_rgb(*v.red(), *v.green(), *v.blue());
                    (28, "workbench_canvas")
                }
            };
            if seen & (1u32 << index) != 0 {
                return Err(LocalLookRefusal::DuplicateField(name));
            }
            seen |= 1u32 << index;
            names.push(name);
        }
        Ok(names)
    }
    pub fn finish(
        self,
        author: &crate::generated::LookVisualFacetOptionSome,
        identity: &str,
    ) -> Result<LookVisualFacetOption, conduit_plot::rust_binding::NativeBindingRefusal> {
        let expression = VisualExpressionInfo::new(
            self.density,
            identity.into(),
            self.motion,
            *author.expression().palette(),
            self.text_scale_percent,
            self.typography,
            2,
        )?;
        let tokens = LookVisualTokensInfo::new(
            self.background,
            self.bootstrap_surface,
            self.emphasis,
            self.failure,
            self.focus,
            self.focus_width_px,
            self.line_height_percent,
            self.muted,
            self.radius_control_px,
            self.radius_panel_px,
            self.reading_paper,
            self.space_control_block_px,
            self.space_control_inline_px,
            self.space_panel_px,
            self.space_unit_px,
            self.structure_primary,
            self.structure_secondary,
            self.success,
            self.surface,
            self.text_primary,
            self.text_secondary,
            self.type_body_px,
            self.type_small_px,
            self.warning,
            self.workbench_canvas,
        )?;
        LookVisualFacetOption::some(expression, tokens)
    }
}

pub(super) fn known(field: &str) -> bool {
    const FIELDS: &[&str] = &[
        "background",
        "reading_paper",
        "workbench_canvas",
        "bootstrap_surface",
        "surface",
        "structure_primary",
        "structure_secondary",
        "text_primary",
        "text_secondary",
        "emphasis",
        "focus",
        "warning",
        "failure",
        "success",
        "muted",
        "type_body_px",
        "type_small_px",
        "line_height_percent",
        "space_unit_px",
        "space_control_inline_px",
        "space_control_block_px",
        "space_panel_px",
        "radius_control_px",
        "radius_panel_px",
        "focus_width_px",
        "text_scale_percent",
        "density",
        "typography",
        "motion",
    ];
    FIELDS.contains(&field)
}
