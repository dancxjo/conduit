//! Native graphical encounter of one exact Face. No application state or dispatch.
//!
//! Preparation retains the complete Face and admits a finite readable document.
//! Pages and focus belong to this Mask; an ordinary Mask executor must still
//! acknowledge scanout before using the returned mappings to make interactions.
use alloc::{string::String, vec::Vec};
use conduit_face::{
    Face, FaceAction, FaceActionArgument, FaceContentId, FaceInteraction, FaceInteractionArgument,
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect, MaskShow,
};

use crate::display::{SPACE_SM, SPACE_XL, typography::TextLayout};

#[path = "native_face_scene/chrome.rs"]
mod chrome;
#[path = "native_face_scene/connected_layout.rs"]
mod connected_layout;
#[path = "native_face_scene/diagram.rs"]
mod diagram;
#[path = "native_face_scene/document.rs"]
mod document;
#[path = "native_face_scene/frame.rs"]
mod frame;
#[path = "native_face_scene/input.rs"]
mod input;
#[path = "native_face_scene/layout.rs"]
mod layout;
#[path = "native_face_scene/ports.rs"]
mod ports;
#[path = "native_face_scene/text_editor.rs"]
mod text_editor;
pub use input::{FaceFocusRequest, FaceSceneInput};
#[cfg(test)]
#[path = "native_face_scene/tests.rs"]
pub(crate) mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceSceneError {
    InvalidFace,
    DocumentBound,
    InvalidExtent,
    SceneBound,
    StaleFace,
    UnknownControl,
    UnavailableAction,
    Interaction,
    UnsupportedInput,
    InputBound,
    IncompleteInput,
    InputUnavailable,
}

/// An index into this immutable Face, never a global or application action code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceControl {
    pub action: usize,
    pub argument: Option<usize>,
}

/// A visible control borrows its exact semantic action and argument contract.
#[derive(Debug)]
pub struct FaceHit<'a> {
    pub bounds: LayoutRect,
    pub control: FaceControl,
    pub action: &'a FaceAction,
    pub argument: Option<&'a FaceActionArgument>,
}

pub struct FaceFrame<'a> {
    pub scene: GraphicsScene,
    pub face_id: &'a FaceContentId,
    pub revision: u64,
    pub page: usize,
    pub pages: usize,
    pub hits: Vec<FaceHit<'a>>,
}

impl FaceFrame<'_> {
    pub fn hit_test(&self, x: i16, y: i16) -> Option<&FaceHit<'_>> {
        self.hits.iter().find(|hit| {
            let bounds = hit.bounds;
            i32::from(x) >= i32::from(bounds.x)
                && i32::from(y) >= i32::from(bounds.y)
                && i32::from(x) < i32::from(bounds.x) + i32::from(bounds.width)
                && i32::from(y) < i32::from(bounds.y) + i32::from(bounds.height)
        })
    }
}

/// One admitted display extent. Changing extent prepares a new scene document;
/// it cannot reinterpret, replace, or mutate its retained semantic Face.
pub struct NativeFaceScene {
    face: Face,
    screen: LayoutRect,
    text_scale_percent: u16,
    primary: Vec<layout::Row>,
    details: Vec<layout::Row>,
    page: usize,
    focus: Option<FaceControl>,
    showing_details: bool,
    showing_diagram: bool,
    input: input::InputState,
    interaction_admitted: bool,
    local_notice: Option<String>,
}

impl NativeFaceScene {
    pub fn prepare(face: Face, width: u16, height: u16) -> Result<Self, FaceSceneError> {
        Self::prepare_with_input(face, width, height, true)
    }

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
        let (primary, details) = document::prepare(&face, interaction_admitted)?;
        Ok(Self {
            primary: layout::admit(primary, screen)?,
            details: layout::admit(details, screen)?,
            face,
            screen,
            text_scale_percent: 100,
            page: 0,
            focus: None,
            showing_details: false,
            showing_diagram: false,
            input: input::InputState::default(),
            interaction_admitted,
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
            item.region = item
                .subject_index
                .and_then(|subject| recipe.subject_regions.get(subject).copied().flatten())
                .and_then(|region| recipe.recipe.regions().as_slice().get(region))
                .map(|region| *region.group());
        }
        let scale = expression.expression.text_scale_percent;
        let padding = match expression.expression.density {
            conduit_face::VisualDensity::Compact => SPACE_SM,
            conduit_face::VisualDensity::Spacious => SPACE_SM * 2,
        };
        scene.primary = layout::admit_with_expression(
            primary,
            scene.screen,
            geometry.gutter,
            geometry.columns,
            scale,
            padding,
        )?;
        scene.details = layout::admit_with_expression(
            details,
            scene.screen,
            geometry.gutter,
            geometry.columns,
            scale,
            padding,
        )?;
        scene.text_scale_percent = scale;
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
        let padding = match expression.expression.density {
            conduit_face::VisualDensity::Compact => SPACE_SM,
            conduit_face::VisualDensity::Spacious => SPACE_SM * 2,
        };
        scene.primary =
            layout::admit_with_expression(primary, scene.screen, SPACE_XL, 1, scale, padding)?;
        scene.details =
            layout::admit_with_expression(details, scene.screen, SPACE_XL, 1, scale, padding)?;
        scene.text_scale_percent = scale;
        Ok(scene)
    }

    /// The selected renderer must use this same scale for rasterization.
    pub fn text_scale_percent(&self) -> u16 {
        self.text_scale_percent
    }

    pub fn face(&self) -> &Face {
        &self.face
    }

    /// Bounded Mask-local outcome text. It never changes the owner's Face.
    pub fn set_local_notice(&mut self, notice: &str) -> Result<(), FaceSceneError> {
        if notice.is_empty()
            || notice.len() > 96
            || !notice.bytes().all(|byte| (32..=126).contains(&byte))
        {
            return Err(FaceSceneError::InputBound);
        }
        chrome::Chrome::prepare(self.screen, self.text_scale_percent)?
            .admit_footer(notice, self.text_scale_percent)?;
        self.local_notice = Some(notice.into());
        Ok(())
    }

    pub fn clear_local_notice(&mut self) -> bool {
        self.local_notice.take().is_some()
    }

    pub fn focused(&self) -> Option<FaceControl> {
        self.focus
    }

    pub fn showing_details(&self) -> bool {
        self.showing_details
    }

    pub fn showing_diagram(&self) -> bool {
        self.showing_diagram
    }

    pub fn has_diagram(&self) -> bool {
        diagram::exists(&self.face)
    }

    pub fn pages(&self) -> usize {
        if self.showing_diagram {
            diagram::pages(&self.face, self.screen.width, self.screen.height)
        } else {
            self.rows().last().map_or(1, |row| row.page + 1)
        }
    }

    pub fn page(&self) -> usize {
        self.page
    }

    pub fn show_details(&mut self, details: bool) {
        self.showing_details = details;
        self.showing_diagram = false;
        // Preserve the exact control across reading views when it is present;
        // moving to Details must not silently move the user's selection.
        let retained = self.focus.and_then(|control| {
            self.rows()
                .iter()
                .find(|row| row.control == Some(control))
                .map(|row| (control, row.page))
        });
        self.focus = retained.map(|(control, _)| control);
        self.page = retained.map_or(0, |(_, page)| page);
    }

    /// A diagram is available only when Gear subjects actually occur in this
    /// exact Face; F3 never constructs a specimen graph to fill an empty view.
    pub fn show_diagram(&mut self, diagram: bool) {
        self.showing_diagram = diagram && self.has_diagram();
        self.page = 0;
        self.focus = None;
    }

    /// Mask-local reading navigation does not invoke a semantic action.
    pub fn turn_page(&mut self, forward: bool) {
        self.page = if forward {
            (self.page + 1).min(self.pages() - 1)
        } else {
            self.page.saturating_sub(1)
        };
        self.focus = None;
    }

    pub fn focus_next(&mut self, forward: bool) -> Option<FaceControl> {
        if self.showing_diagram {
            return None;
        }
        let position = self.focus.and_then(|focus| {
            self.rows()
                .iter()
                .position(|row| row.control == Some(focus))
        });
        let mut controls = self.rows().iter().enumerate().filter_map(|(index, row)| {
            let control = row.control?;
            self.face.actions[control.action]
                .availability
                .is_available()
                .then_some((index, control, row.page))
        });
        let selected = if forward {
            controls
                .find(|(index, control, _)| {
                    position.is_none_or(|value| *index > value) && Some(*control) != self.focus
                })
                .map(|(_, control, page)| (control, page))
                .or_else(|| self.rows().iter().find_map(|row| self.eligible(row)))
        } else {
            controls
                .rev()
                .find(|(index, control, _)| {
                    position.is_none_or(|value| *index < value) && Some(*control) != self.focus
                })
                .map(|(_, control, page)| (control, page))
                .or_else(|| self.rows().iter().rev().find_map(|row| self.eligible(row)))
        };
        if let Some((control, page)) = selected {
            self.focus = Some(control);
            self.page = page;
        }
        self.focus
    }

    /// Resolve a retained frame mapping only against its exact Face identity.
    pub fn resolve(
        &self,
        face_id: &FaceContentId,
        revision: u64,
        control: FaceControl,
    ) -> Result<(&FaceAction, Option<&FaceActionArgument>), FaceSceneError> {
        if face_id != &self.face.identity || revision != self.face.revision {
            return Err(FaceSceneError::StaleFace);
        }
        let action = self
            .face
            .actions
            .get(control.action)
            .ok_or(FaceSceneError::UnknownControl)?;
        if !action.availability.is_available() {
            return Err(FaceSceneError::UnavailableAction);
        }
        let argument = control
            .argument
            .map(|index| {
                action
                    .arguments
                    .get(index)
                    .ok_or(FaceSceneError::UnknownControl)
            })
            .transpose()?;
        Ok((action, argument))
    }

    /// No default values, inferred authority, or draft edits occur here. All
    /// arguments are supplied atomically and validated by the existing boundary.
    pub fn interaction(
        &self,
        face_id: &FaceContentId,
        revision: u64,
        control: FaceControl,
        show: &MaskShow,
        arguments: Vec<FaceInteractionArgument>,
        sequence: u64,
    ) -> Result<FaceInteraction, FaceSceneError> {
        if !self.interaction_admitted {
            return Err(FaceSceneError::InputUnavailable);
        }
        let (action, _) = self.resolve(face_id, revision, control)?;
        FaceInteraction::new(
            &self.face,
            show,
            &action.identity,
            &action.target,
            arguments,
            sequence,
        )
        .map_err(|_| FaceSceneError::Interaction)
    }

    fn eligible(&self, row: &layout::Row) -> Option<(FaceControl, usize)> {
        if !self.interaction_admitted {
            return None;
        }
        let control = row.control?;
        self.face.actions[control.action]
            .availability
            .is_available()
            .then_some((control, row.page))
    }

    fn rows(&self) -> &[layout::Row] {
        if self.showing_details {
            &self.details
        } else {
            &self.primary
        }
    }
}

fn push(
    scene: &mut GraphicsScene,
    command: Result<GraphicsCommand, conduit_face::GraphicsError>,
) -> Result<(), FaceSceneError> {
    scene
        .push(command.map_err(|_| FaceSceneError::SceneBound)?)
        .map_err(|_| FaceSceneError::SceneBound)
}

#[cfg(test)]
#[path = "native_face_scene/open_domain_tests.rs"]
mod open_domain_tests;

#[cfg(test)]
#[path = "native_face_scene/measured_layout_tests.rs"]
mod measured_layout_tests;

#[cfg(test)]
#[path = "native_face_scene/raster_layout_tests.rs"]
pub(crate) mod raster_layout_tests;

#[cfg(test)]
#[path = "native_face_scene/navigation_tests.rs"]
mod navigation_tests;
