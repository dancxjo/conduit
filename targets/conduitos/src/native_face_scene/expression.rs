//! Admit selected visual metrics before publishing native geometry or hits.
use super::*;

impl NativeFaceScene {
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
        let typography = Typography::new(expression.expression.typography);
        let padding = match expression.expression.density {
            conduit_face::VisualDensity::Compact => SPACE_SM,
            conduit_face::VisualDensity::Spacious => SPACE_SM * 2,
        };
        let panel_groups = layout::authored_panel_groups(
            recipe.recipe.regions().iter().map(|region| *region.group()),
        );
        scene.primary = layout::admit_configured_selected(
            primary,
            scene.screen,
            layout::ConfiguredColumns {
                gutter: geometry.gutter,
                columns: geometry.columns,
                padding,
                groups: panel_groups,
            },
            scale,
            typography,
        )?;
        scene.details = layout::admit_with_expression_selected(
            details,
            scene.screen,
            geometry.gutter,
            geometry.columns,
            scale,
            padding,
            typography,
        )?;
        scene.text_scale_percent = scale;
        scene.typography = typography;
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
        let mut scene = Self::prepare_with_input(face, width, height, interaction_admitted)?;
        let (primary, details) = document::prepare(&scene.face, interaction_admitted)?;
        let scale = expression.expression.text_scale_percent;
        let typography = Typography::new(expression.expression.typography);
        let padding = match expression.expression.density {
            conduit_face::VisualDensity::Compact => SPACE_SM,
            conduit_face::VisualDensity::Spacious => SPACE_SM * 2,
        };
        scene.primary = layout::admit_with_expression_selected(
            primary,
            scene.screen,
            SPACE_XL,
            1,
            scale,
            padding,
            typography,
        )?;
        scene.details = layout::admit_with_expression_selected(
            details,
            scene.screen,
            SPACE_XL,
            1,
            scale,
            padding,
            typography,
        )?;
        scene.text_scale_percent = scale;
        scene.typography = typography;
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
        let face = structure.face();
        face.validate().map_err(|_| FaceSceneError::InvalidFace)?;
        if width < 240 || height < 240 || width > 16_384 || height > 16_384 {
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
        let typography = Typography::new(expression.expression.typography);
        let padding = match expression.expression.density {
            conduit_face::VisualDensity::Compact => SPACE_SM,
            conduit_face::VisualDensity::Spacious => SPACE_SM * 2,
        };
        let primary =
            layout::admit_mechanical(primary, screen, structure, scale, padding, typography)?;
        let details = layout::admit_with_expression_selected(
            details, screen, SPACE_XL, 1, scale, padding, typography,
        )?;
        Ok(Self {
            primary,
            details,
            graph_reading: graph_reading::GraphReading::prepare(face)?,
            face: face.clone(),
            screen,
            text_scale_percent: scale,
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
