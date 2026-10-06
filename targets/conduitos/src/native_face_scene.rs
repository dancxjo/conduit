//! Native graphical encounter of one exact Face. No application state or dispatch.
//!
//! Preparation retains the complete Face and admits a finite readable document.
//! Pages and focus belong to this Mask; an ordinary Mask executor must still
//! acknowledge scanout before using the returned mappings to make interactions.
use alloc::{string::String, vec::Vec};
use conduit_presentation::{
    FaceActionArgument, FaceInteraction, FaceInteractionArgument, GraphicsCommand,
    GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole, LayoutRect, MaskShow,
    Presentation, PresentationAction, PresentationContentId,
};

use crate::display::{SPACE_SM, SPACE_XL, typography::TextLayout};

#[path = "native_face_scene/diagram.rs"]
mod diagram;
#[path = "native_face_scene/document.rs"]
mod document;
#[path = "native_face_scene/input.rs"]
mod input;
#[path = "native_face_scene/layout.rs"]
mod layout;
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
    pub action: &'a PresentationAction,
    pub argument: Option<&'a FaceActionArgument>,
}

pub struct FaceFrame<'a> {
    pub scene: GraphicsScene,
    pub presentation_id: &'a PresentationContentId,
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
    face: Presentation,
    screen: LayoutRect,
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
    pub fn prepare(face: Presentation, width: u16, height: u16) -> Result<Self, FaceSceneError> {
        Self::prepare_with_input(face, width, height, true)
    }

    pub fn prepare_with_input(
        face: Presentation,
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
            page: 0,
            focus: None,
            showing_details: false,
            showing_diagram: false,
            input: input::InputState::default(),
            interaction_admitted,
            local_notice: None,
        })
    }

    pub fn presentation(&self) -> &Presentation {
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
        self.page = 0;
        self.focus = None;
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
        presentation_id: &PresentationContentId,
        revision: u64,
        control: FaceControl,
    ) -> Result<(&PresentationAction, Option<&FaceActionArgument>), FaceSceneError> {
        if presentation_id != &self.face.identity || revision != self.face.revision {
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
        presentation_id: &PresentationContentId,
        revision: u64,
        control: FaceControl,
        show: &MaskShow,
        arguments: Vec<FaceInteractionArgument>,
        sequence: u64,
    ) -> Result<FaceInteraction, FaceSceneError> {
        if !self.interaction_admitted {
            return Err(FaceSceneError::InputUnavailable);
        }
        let (action, _) = self.resolve(presentation_id, revision, control)?;
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

    pub fn frame(&self) -> Result<FaceFrame<'_>, FaceSceneError> {
        let mut scene = GraphicsScene::empty();
        push(
            &mut scene,
            GraphicsCommand::rect(
                self.screen,
                self.screen,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            ),
        )?;
        let heading = if !self.interaction_admitted && self.showing_diagram {
            "OWNER FACE / PATCHBAY"
        } else if !self.interaction_admitted && self.showing_details {
            "OWNER FACE / DETAILS"
        } else if !self.interaction_admitted {
            "OWNER FACE / SNAPSHOT"
        } else if self.showing_diagram {
            "PATCHBAY"
        } else if self.showing_details {
            "DETAILS"
        } else {
            "CURRENT VIEW"
        };
        push(
            &mut scene,
            GraphicsCommand::text(
                LayoutRect {
                    x: SPACE_XL as i16,
                    y: 12,
                    width: self.screen.width - SPACE_XL * 2 - 88,
                    height: 32,
                },
                self.screen,
                GraphicsPaintRole::Accent,
                heading,
            )
            .and_then(|command| command.with_text_role(GraphicsTextRole::Heading)),
        )?;
        let page_marker = alloc::format!("{} / {}", self.page + 1, self.pages());
        push(
            &mut scene,
            GraphicsCommand::text(
                LayoutRect {
                    x: (self.screen.width - SPACE_XL - 80) as i16,
                    y: 16,
                    width: 80,
                    height: 24,
                },
                self.screen,
                GraphicsPaintRole::Muted,
                &page_marker,
            )
            .and_then(|command| command.with_text_role(GraphicsTextRole::Label)),
        )?;
        push(
            &mut scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: SPACE_XL as i16,
                    y: (layout::HEADER - 8) as i16,
                    width: self.screen.width - SPACE_XL * 2,
                    height: 2,
                },
                self.screen,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Fill,
            ),
        )?;
        let mut hits = Vec::with_capacity(layout::MAX_PAGE_ROWS);
        if self.showing_diagram {
            diagram::append(&mut scene, &self.face, self.screen, self.page)?;
        }
        for row in self
            .rows()
            .iter()
            .filter(|row| !self.showing_diagram && row.page == self.page)
        {
            let available = self.interaction_admitted
                && row.control.is_some_and(|control| {
                    self.face.actions[control.action]
                        .availability
                        .is_available()
                });
            let outline = if available && row.control == self.focus {
                GraphicsPaintRole::Focus
            } else {
                row.paint
            };
            layout::append_row_chrome(&mut scene, row, self.screen, outline, self.showing_details)?;
            push(
                &mut scene,
                GraphicsCommand::text(row.text_bounds(), self.screen, row.paint, &row.text)
                    .and_then(|command| command.with_text_role(row.role)),
            )?;
            if available {
                let control = row.control.ok_or(FaceSceneError::UnknownControl)?;
                let action = &self.face.actions[control.action];
                hits.push(FaceHit {
                    bounds: row.bounds,
                    control,
                    action,
                    argument: control.argument.map(|index| &action.arguments[index]),
                });
            }
        }
        let footer = self
            .local_notice
            .clone()
            .or_else(|| self.input.preview(self.focus))
            .unwrap_or_else(|| {
                if !self.interaction_admitted && self.has_diagram() {
                    "Read only · PgUp/PgDn pages · F2 facts · F3 Patchbay".into()
                } else if !self.interaction_admitted {
                    "Read only · PgUp/PgDn pages · F2 facts".into()
                } else if self.showing_diagram {
                    "Visible links only · F2 all facts · F3 return".into()
                } else if self.showing_details {
                    "PgUp/PgDn pages · Tab next · F2 return".into()
                } else if self.has_diagram() {
                    "Tab next · Enter act · F2 inspect · F3 Patchbay".into()
                } else {
                    "PgUp/PgDn pages · Tab next · Enter act · F2 inspect".into()
                }
            });
        push(
            &mut scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: SPACE_XL as i16,
                    y: (self.screen.height - layout::FOOTER + 4) as i16,
                    width: self.screen.width - SPACE_XL * 2,
                    height: 2,
                },
                self.screen,
                GraphicsPaintRole::Muted,
                GraphicsShapeStyle::Fill,
            ),
        )?;
        let bounds = LayoutRect {
            x: SPACE_XL as i16,
            y: (self.screen.height - layout::FOOTER + 14) as i16,
            width: self.screen.width - SPACE_XL * 2,
            height: layout::FOOTER - 16,
        };
        push(
            &mut scene,
            GraphicsCommand::text(bounds, self.screen, GraphicsPaintRole::Muted, &footer)
                .and_then(|command| command.with_text_role(GraphicsTextRole::Muted)),
        )?;
        Ok(FaceFrame {
            scene,
            presentation_id: &self.face.identity,
            revision: self.face.revision,
            page: self.page,
            pages: self.pages(),
            hits,
        })
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
    command: Result<GraphicsCommand, conduit_presentation::GraphicsError>,
) -> Result<(), FaceSceneError> {
    scene
        .push(command.map_err(|_| FaceSceneError::SceneBound)?)
        .map_err(|_| FaceSceneError::SceneBound)
}
