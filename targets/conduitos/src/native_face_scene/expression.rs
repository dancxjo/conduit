//! Admit selected visual metrics before publishing native geometry or hits.
use super::*;

struct SelectedMetrics {
    typography: Typography,
    row_gap: u16,
    control: Option<layout::RowInsets>,
    panel: Option<u16>,
    focus_marker_width: u16,
    corners: Option<corners::LookCorners>,
}
impl SelectedMetrics {
    fn legacy(typography: Typography) -> Self {
        Self {
            typography,
            row_gap: SPACE_SM,
            control: None,
            panel: None,
            focus_marker_width: 2,
            corners: None,
        }
    }
    fn spacing(&self, expression: &conduit_face::AdmittedVisualExpression) -> layout::RowSpacing {
        layout::RowSpacing {
            padding: match expression.expression.density {
                conduit_face::VisualDensity::Compact => SPACE_SM,
                conduit_face::VisualDensity::Spacious => SPACE_SM * 2,
            },
            gap: self.row_gap,
            control: self.control,
            panel: self.panel,
        }
    }
}

impl NativeFaceScene {
    pub fn prepare_with_input(
        face: Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
    ) -> Result<Self, FaceSceneError> {
        face.validate().map_err(|_| FaceSceneError::InvalidFace)?;
        if width < 320 || height < 240 || width > 16_384 || height > 16_384 {
            return Err(FaceSceneError::InvalidExtent);
        }
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let (mut primary, details) = document::prepare(&face, interaction_admitted)?;
        let spacing = layout::RowSpacing::legacy(SPACE_SM);
        let viewport = layout::Viewport::for_face(
            screen,
            100,
            Typography::EDITORIAL,
            spacing,
            &face,
            interaction_admitted,
            [&primary, &details],
        )?;
        document::omit_encounter_heading(&mut primary, &face, &viewport.chrome, 100);
        Ok(Self {
            primary: layout::admit_with_expression_selected(
                primary, SPACE_XL, 1, spacing, &viewport,
            )?,
            details: layout::admit_with_expression_selected(
                details, SPACE_XL, 1, spacing, &viewport,
            )?,
            graph_reading: graph_reading::GraphReading::prepare(&face)?,
            face,
            screen,
            text_scale_percent: 100,
            header_top: viewport.chrome.top,
            focus_marker_width: 2,
            look_corners: None,
            typography: Typography::EDITORIAL,
            page: 0,
            focus: None,
            showing_details: false,
            showing_diagram: false,
            input: input::InputState::default(),
            interaction_admitted,
            local_input_available: true,
            local_creche_preferences: false,
            local_notice: None,
        })
    }

    /// Realize source-owned structural geometry and the admitted visual facet
    /// over this exact Face. Layout retains all original control references.
    pub fn prepare_configured(
        face: Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        recipe: &conduit_face::PreparedComponentRecipe,
        expression: &conduit_face::AdmittedVisualExpression,
    ) -> Result<Self, FaceSceneError> {
        Self::prepare_configured_typography(
            face,
            (width, height),
            interaction_admitted,
            recipe,
            expression,
            SelectedMetrics::legacy(Typography::new(expression.expression.typography)),
        )
    }

    /// Prepare configured geometry with the selected Look's admitted text metrics.
    pub fn prepare_configured_look(
        face: Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        recipe: &conduit_face::PreparedComponentRecipe,
        look: &conduit_face::AdmittedVisualLook,
    ) -> Result<Self, FaceSceneError> {
        Self::prepare_configured_typography(
            face,
            (width, height),
            interaction_admitted,
            recipe,
            &look.visual,
            Self::look_metrics(look)?,
        )
    }

    fn prepare_configured_typography(
        face: Face,
        extent: (u16, u16),
        interaction_admitted: bool,
        recipe: &conduit_face::PreparedComponentRecipe,
        expression: &conduit_face::AdmittedVisualExpression,
        metrics: SelectedMetrics,
    ) -> Result<Self, FaceSceneError> {
        let typography = metrics.typography;
        let (width, height) = extent;
        if recipe.reading.source_face_identity != face.identity.as_str()
            || recipe.reading.source_face_revision != face.revision
        {
            return Err(FaceSceneError::StaleFace);
        }
        let geometry = recipe
            .geometry_for_width(width)
            .map_err(|_| FaceSceneError::InvalidExtent)?;
        let mut scene = Self::prepare_with_input(face, width, height, interaction_admitted)?;
        let (mut primary, details) = document::prepare(&scene.face, interaction_admitted)?;
        for item in &mut primary {
            item.region_order = item
                .subject_index
                .and_then(|subject| recipe.subject_regions.get(subject).copied().flatten());
            item.region = item
                .subject_index
                .and_then(|subject| recipe.subject_regions.get(subject).copied().flatten())
                .and_then(|region| recipe.recipe.regions().as_slice().get(region))
                .map(|region| *region.group());
        }
        let scale = expression.expression.text_scale_percent;
        let spacing = metrics.spacing(expression);
        let viewport = layout::Viewport::for_face(
            scene.screen,
            scale,
            typography,
            spacing,
            &scene.face,
            interaction_admitted,
            [&primary, &details],
        )?;
        document::omit_encounter_heading(&mut primary, &scene.face, &viewport.chrome, scale);
        let panel_groups = layout::authored_panel_groups(
            recipe.recipe.regions().iter().map(|region| *region.group()),
        );
        scene.primary = layout::admit_configured_selected(
            primary,
            layout::ConfiguredColumns {
                gutter: geometry.gutter,
                columns: geometry.columns,
                spacing,
                groups: panel_groups,
            },
            &viewport,
        )?;
        scene.details = layout::admit_with_expression_selected(
            details,
            geometry.gutter,
            geometry.columns,
            spacing,
            &viewport,
        )?;
        scene.header_top = viewport.chrome.top;
        scene.text_scale_percent = scale;
        scene.typography = typography;
        scene.focus_marker_width = metrics.focus_marker_width;
        scene.look_corners = metrics.corners;
        Ok(scene)
    }

    /// Generic semantic projection when configured structural capacity is unavailable.
    /// User text requirements still use the same measured and rasterized scale.
    pub fn prepare_visual_fallback(
        face: Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        expression: &conduit_face::AdmittedVisualExpression,
    ) -> Result<Self, FaceSceneError> {
        Self::prepare_visual_typography(
            face,
            width,
            height,
            interaction_admitted,
            expression,
            SelectedMetrics::legacy(Typography::new(expression.expression.typography)),
        )
    }
    /// Realize the selected immutable Look's existing typed size/leading tokens.
    /// Paint must consume this same selected Look/typography and admitted scale.
    pub fn prepare_visual_look(
        face: Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        look: &conduit_face::AdmittedVisualLook,
    ) -> Result<Self, FaceSceneError> {
        let metrics = Self::look_metrics(look)?;
        Self::prepare_visual_typography(
            face,
            width,
            height,
            interaction_admitted,
            &look.visual,
            metrics,
        )
    }
    fn look_metrics(
        look: &conduit_face::AdmittedVisualLook,
    ) -> Result<SelectedMetrics, FaceSceneError> {
        let typography =
            Typography::with_visual_look(look.visual.expression.typography, &look.selected, 200)
                .map_err(FaceSceneError::LookTypography)?;
        let conduit_face::LookVisualFacetOption::Some(visual) = look.selected.visual() else {
            return Err(FaceSceneError::InvalidExtent);
        };
        let row_gap = *visual.tokens().space_unit_px();
        if row_gap > 32 {
            return Err(FaceSceneError::InvalidExtent);
        }
        Ok(SelectedMetrics {
            typography,
            row_gap,
            focus_marker_width: *visual.tokens().focus_width_px(),
            corners: Some(corners::LookCorners::from_tokens(visual.tokens())?),
            panel: Some(*visual.tokens().space_panel_px()),
            control: Some(layout::RowInsets {
                inline: *visual.tokens().space_control_inline_px(),
                block: *visual.tokens().space_control_block_px(),
                leading: 8,
            }),
        })
    }

    fn prepare_visual_typography(
        face: Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        expression: &conduit_face::AdmittedVisualExpression,
        metrics: SelectedMetrics,
    ) -> Result<Self, FaceSceneError> {
        let typography = metrics.typography;
        let mut scene = Self::prepare_with_input(face, width, height, interaction_admitted)?;
        let (mut primary, details) = document::prepare(&scene.face, interaction_admitted)?;
        let scale = expression.expression.text_scale_percent;
        let spacing = metrics.spacing(expression);
        let viewport = layout::Viewport::for_face(
            scene.screen,
            scale,
            typography,
            spacing,
            &scene.face,
            interaction_admitted,
            [&primary, &details],
        )?;
        document::omit_encounter_heading(&mut primary, &scene.face, &viewport.chrome, scale);
        scene.primary =
            layout::admit_with_expression_selected(primary, SPACE_XL, 1, spacing, &viewport)?;
        scene.details =
            layout::admit_with_expression_selected(details, SPACE_XL, 1, spacing, &viewport)?;
        scene.header_top = viewport.chrome.top;
        scene.text_scale_percent = scale;
        scene.typography = typography;
        scene.focus_marker_width = metrics.focus_marker_width;
        scene.look_corners = metrics.corners;
        Ok(scene)
    }
}

impl NativeFaceScene {
    /// Consume checked structure over its exact immutable Face. This entrypoint
    /// alone does not establish selected Back custody or Show acknowledgement.
    pub fn prepare_structured(
        structure: &conduit_face::PreparedComponentLayout<'_>,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        expression: &conduit_face::AdmittedVisualExpression,
    ) -> Result<Self, FaceSceneError> {
        Self::prepare_structured_typography(
            structure,
            width,
            height,
            interaction_admitted,
            expression,
            SelectedMetrics::legacy(Typography::new(expression.expression.typography)),
        )
    }

    /// Apply selected Look metrics without changing checked mechanical structure.
    pub fn prepare_structured_look(
        structure: &conduit_face::PreparedComponentLayout<'_>,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        look: &conduit_face::AdmittedVisualLook,
    ) -> Result<Self, FaceSceneError> {
        Self::prepare_structured_typography(
            structure,
            width,
            height,
            interaction_admitted,
            &look.visual,
            Self::look_metrics(look)?,
        )
    }

    fn prepare_structured_typography(
        structure: &conduit_face::PreparedComponentLayout<'_>,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        expression: &conduit_face::AdmittedVisualExpression,
        metrics: SelectedMetrics,
    ) -> Result<Self, FaceSceneError> {
        let typography = metrics.typography;
        let face = structure.face();
        face.validate().map_err(|_| FaceSceneError::InvalidFace)?;
        if width < 320 || height < 240 || width > 16_384 || height > 16_384 {
            return Err(FaceSceneError::InvalidExtent);
        }
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        };
        // Prepare the selected layout directly. A generic scene's geometry is
        // neither its prerequisite nor storage that should coexist with it.
        let (mut primary, details) = document::prepare(face, interaction_admitted)?;
        for item in &mut primary {
            item.region_order = item.subject_index.and_then(|subject| {
                structure
                    .recipe()
                    .subject_regions
                    .get(subject)
                    .copied()
                    .flatten()
            });
            item.region = item
                .region_order
                .and_then(|region| structure.recipe().recipe.regions().as_slice().get(region))
                .map(|region| *region.group());
        }
        let scale = expression.expression.text_scale_percent;
        let spacing = metrics.spacing(expression);
        let viewport = layout::Viewport::for_face(
            screen,
            scale,
            typography,
            spacing,
            face,
            interaction_admitted,
            [&primary, &details],
        )?;
        document::omit_encounter_heading(&mut primary, face, &viewport.chrome, scale);
        let primary = layout::admit_mechanical(primary, structure, spacing, &viewport)?;
        let details =
            layout::admit_with_expression_selected(details, SPACE_XL, 1, spacing, &viewport)?;
        Ok(Self {
            primary,
            details,
            graph_reading: graph_reading::GraphReading::prepare(face)?,
            face: face.clone(),
            screen,
            text_scale_percent: scale,
            header_top: viewport.chrome.top,
            focus_marker_width: metrics.focus_marker_width,
            look_corners: metrics.corners,
            typography,
            page: 0,
            focus: None,
            showing_details: false,
            showing_diagram: false,
            input: input::InputState::default(),
            interaction_admitted,
            local_input_available: true,
            local_creche_preferences: false,
            local_notice: None,
        })
    }
}
