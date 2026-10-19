//! Assemble graphics and exact Face hits from an already admitted document.
use super::*;

impl NativeFaceScene {
    /// Actual admitted display bounds, for read-only cursor/hit observation.
    pub fn bounds(&self) -> LayoutRect {
        self.screen
    }

    pub fn frame(&self) -> Result<FaceFrame<'_>, FaceSceneError> {
        let chrome = chrome::Chrome::prepare(self.screen, self.text_scale_percent)?;
        if self.showing_diagram
            && (chrome.top != layout::HEADER
                || self.screen.height - chrome.bottom != layout::FOOTER)
        {
            // Independent Patchbay topology retains its admitted geometry.
            // The same Face remains readable through F2/Details.
            return Err(FaceSceneError::InvalidExtent);
        }
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
        let heading = if chrome.compact {
            if self.showing_diagram {
                "GRAPH"
            } else if self.showing_details {
                "DETAILS"
            } else {
                "VIEW"
            }
        } else if !self.interaction_admitted && self.showing_diagram {
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
                chrome.heading,
                self.screen,
                GraphicsPaintRole::Accent,
                heading,
            )
            .and_then(|command| command.with_text_role(chrome.heading_role)),
        )?;
        let page_marker = if chrome.compact {
            alloc::format!("{}/{}", self.page + 1, self.pages())
        } else {
            alloc::format!("{} / {}", self.page + 1, self.pages())
        };
        push(
            &mut scene,
            GraphicsCommand::text(
                chrome.marker,
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
                    y: (chrome.top - 4) as i16,
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
        let editor_visible = self.interaction_admitted
            && !self.showing_details
            && !self.showing_diagram
            && text_editor::append(&mut scene, &self.face, self.screen, self.text_scale_percent)?;
        let mut focus_marked = false;
        for row in self
            .rows()
            .iter()
            .filter(|row| !editor_visible && !self.showing_diagram && row.page == self.page)
        {
            let available = self.interaction_admitted
                && row.control.is_some_and(|control| {
                    self.face.actions[control.action]
                        .availability
                        .is_available()
                });
            let focused = available && row.control == self.focus;
            let outline = if focused {
                GraphicsPaintRole::Focus
            } else {
                row.paint
            };
            layout::append_row_chrome(
                &mut scene,
                row,
                self.screen,
                outline,
                self.showing_details,
                focused && !focus_marked,
            )?;
            focus_marked |= focused;
            super::boolean_choice::append(&mut scene, row, self.screen)?;
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
                if editor_visible {
                    if chrome.compact {
                        chrome::EDITOR_COMPACT_FOOTER.into()
                    } else {
                        chrome::EDITOR_FOOTER.into()
                    }
                } else if chrome.compact && !self.showing_diagram {
                    if self.interaction_admitted {
                        "PgDn · F2".into()
                    } else {
                        "Read only · F2".into()
                    }
                } else if self.showing_diagram {
                    diagram::link_notice(&self.face, self.screen, self.page)
                } else if !self.interaction_admitted && self.has_diagram() {
                    "Read only · PgUp/PgDn pages · F2 facts · F3 Patchbay".into()
                } else if !self.interaction_admitted {
                    "Read only · PgUp/PgDn pages · F2 facts".into()
                } else if self.showing_details {
                    "PgUp/PgDn pages · Tab next · F2 return".into()
                } else if self.has_diagram() {
                    "Tab next · Enter act · F2 inspect · F3 Patchbay".into()
                } else {
                    "PgUp/PgDn pages · Tab next · Enter act · F2 inspect".into()
                }
            });
        chrome.admit_footer(&footer, self.text_scale_percent)?;
        push(
            &mut scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: SPACE_XL as i16,
                    y: (chrome.bottom + 2) as i16,
                    width: self.screen.width - SPACE_XL * 2,
                    height: 2,
                },
                self.screen,
                GraphicsPaintRole::Muted,
                GraphicsShapeStyle::Fill,
            ),
        )?;
        let bounds = chrome.footer;
        push(
            &mut scene,
            GraphicsCommand::text(bounds, self.screen, GraphicsPaintRole::Muted, &footer)
                .and_then(|command| command.with_text_role(GraphicsTextRole::Muted)),
        )?;
        Ok(FaceFrame {
            scene,
            face_id: &self.face.identity,
            revision: self.face.revision,
            page: self.page,
            pages: self.pages(),
            hits,
        })
    }
}
