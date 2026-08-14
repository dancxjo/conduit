//! Finite renderer-consumable allocation for Patchbay's ordinary shell.

use conduit_presentation::LayoutRect;

pub const MAX_PRESENTATION_REGIONS: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationRegionId {
    HeaderMeaning,
    HeaderActions,
    Navigator,
    Canvas,
    Inspector,
    FooterMeaning,
    FooterStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PresentationPriority {
    ExactTruth,
    SelectedDetail,
    SecondaryContext,
    PrimaryMeaning,
    CurrentAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationOverflow {
    ElideInspect,
    Wrap,
    Scroll,
    Disclose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationRegionMode {
    Allocated,
    InspectorDrawer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationRegion {
    pub id: PresentationRegionId,
    pub bounds: LayoutRect,
    pub priority: PresentationPriority,
    pub overflow: PresentationOverflow,
    pub mode: PresentationRegionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutCollision {
    pub first: PresentationRegionId,
    pub second: PresentationRegionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationLayoutError {
    UnsupportedViewport,
    InvalidTextScale,
    ArithmeticOverflow,
    RegionOutsideViewport(PresentationRegionId),
    IllegalCollision(LayoutCollision),
    InvalidMeasurement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasuredTextFit {
    pub visible_characters: usize,
    pub elided: bool,
    pub measured_width: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponsivePatchbayLayout {
    viewport: LayoutRect,
    regions: Vec<PresentationRegion>,
}

impl ResponsivePatchbayLayout {
    pub fn allocate(
        width: u16,
        height: u16,
        text_scale_percent: u16,
        inspector_requested: bool,
    ) -> Result<Self, PresentationLayoutError> {
        if width < 480 || height < 320 {
            return Err(PresentationLayoutError::UnsupportedViewport);
        }
        if !(100..=200).contains(&text_scale_percent) {
            return Err(PresentationLayoutError::InvalidTextScale);
        }
        let scaled = |value: u32| {
            value
                .checked_mul(u32::from(text_scale_percent))
                .and_then(|value| value.checked_add(99))
                .map(|value| value / 100)
                .ok_or(PresentationLayoutError::ArithmeticOverflow)
        };
        let header = scaled(52)?.min(u32::from(height) / 3);
        let footer = scaled(42)?.min(u32::from(height) / 4);
        let content_height = u32::from(height)
            .checked_sub(header)
            .and_then(|value| value.checked_sub(footer))
            .ok_or(PresentationLayoutError::UnsupportedViewport)?;
        let nav = scaled(176)?.min(u32::from(width) / 3);
        let desired_inspector = scaled(284)?.min(u32::from(width) / 2);
        let inspector_allocated = inspector_requested
            && u32::from(width).saturating_sub(nav + desired_inspector) >= scaled(480)?;
        let inspector_width = if inspector_requested {
            desired_inspector
        } else {
            0
        };
        let canvas_width = u32::from(width)
            .checked_sub(nav)
            .and_then(|value| {
                value.checked_sub(if inspector_allocated {
                    inspector_width
                } else {
                    0
                })
            })
            .ok_or(PresentationLayoutError::UnsupportedViewport)?;
        let header_meaning_width = (u32::from(width) * 2 / 5).max(1);
        let footer_meaning_width = (u32::from(width) / 3).max(1);
        let rect = |x: u32, y: u32, width: u32, height: u32| {
            Ok(LayoutRect {
                x: i16::try_from(x).map_err(|_| PresentationLayoutError::ArithmeticOverflow)?,
                y: i16::try_from(y).map_err(|_| PresentationLayoutError::ArithmeticOverflow)?,
                width: u16::try_from(width)
                    .map_err(|_| PresentationLayoutError::ArithmeticOverflow)?,
                height: u16::try_from(height)
                    .map_err(|_| PresentationLayoutError::ArithmeticOverflow)?,
            })
        };
        let mut regions = Vec::with_capacity(MAX_PRESENTATION_REGIONS);
        regions.push(region(
            PresentationRegionId::HeaderMeaning,
            rect(0, 0, header_meaning_width, header)?,
            PresentationPriority::PrimaryMeaning,
            PresentationOverflow::ElideInspect,
        ));
        regions.push(region(
            PresentationRegionId::HeaderActions,
            rect(
                header_meaning_width,
                0,
                u32::from(width) - header_meaning_width,
                header,
            )?,
            PresentationPriority::CurrentAction,
            PresentationOverflow::Wrap,
        ));
        regions.push(region(
            PresentationRegionId::Navigator,
            rect(0, header, nav, content_height)?,
            PresentationPriority::SecondaryContext,
            PresentationOverflow::Scroll,
        ));
        regions.push(region(
            PresentationRegionId::Canvas,
            rect(nav, header, canvas_width, content_height)?,
            PresentationPriority::PrimaryMeaning,
            PresentationOverflow::ElideInspect,
        ));
        if inspector_requested {
            let (x, mode) = if inspector_allocated {
                (
                    u32::from(width) - inspector_width,
                    PresentationRegionMode::Allocated,
                )
            } else {
                (
                    u32::from(width) - inspector_width,
                    PresentationRegionMode::InspectorDrawer,
                )
            };
            regions.push(PresentationRegion {
                id: PresentationRegionId::Inspector,
                bounds: rect(x, header, inspector_width, content_height)?,
                priority: PresentationPriority::SelectedDetail,
                overflow: PresentationOverflow::Scroll,
                mode,
            });
        }
        regions.push(region(
            PresentationRegionId::FooterMeaning,
            rect(0, header + content_height, footer_meaning_width, footer)?,
            PresentationPriority::SecondaryContext,
            PresentationOverflow::ElideInspect,
        ));
        regions.push(region(
            PresentationRegionId::FooterStatus,
            rect(
                footer_meaning_width,
                header + content_height,
                u32::from(width) - footer_meaning_width,
                footer,
            )?,
            PresentationPriority::CurrentAction,
            PresentationOverflow::ElideInspect,
        ));
        let layout = Self {
            viewport: rect(0, 0, u32::from(width), u32::from(height))?,
            regions,
        };
        layout.validate()?;
        Ok(layout)
    }

    pub fn viewport(&self) -> LayoutRect {
        self.viewport
    }

    pub fn regions(&self) -> &[PresentationRegion] {
        &self.regions
    }

    pub fn region(&self, id: PresentationRegionId) -> Option<&PresentationRegion> {
        self.regions.iter().find(|region| region.id == id)
    }

    pub fn validate(&self) -> Result<(), PresentationLayoutError> {
        if self.regions.len() > MAX_PRESENTATION_REGIONS {
            return Err(PresentationLayoutError::ArithmeticOverflow);
        }
        for region in &self.regions {
            if !contains(self.viewport, region.bounds) {
                return Err(PresentationLayoutError::RegionOutsideViewport(region.id));
            }
        }
        for (index, first) in self.regions.iter().enumerate() {
            for second in self.regions.iter().skip(index + 1) {
                if intersects(first.bounds, second.bounds)
                    && first.mode != PresentationRegionMode::InspectorDrawer
                    && second.mode != PresentationRegionMode::InspectorDrawer
                {
                    return Err(PresentationLayoutError::IllegalCollision(LayoutCollision {
                        first: first.id,
                        second: second.id,
                    }));
                }
            }
        }
        Ok(())
    }
}

pub fn fit_measured_text(
    text: &str,
    character_advances: &[u16],
    available_width: u32,
    ellipsis_width: u16,
) -> Result<MeasuredTextFit, PresentationLayoutError> {
    if text.chars().count() != character_advances.len() {
        return Err(PresentationLayoutError::InvalidMeasurement);
    }
    let full_width = character_advances.iter().try_fold(0u32, |sum, width| {
        sum.checked_add(u32::from(*width))
            .ok_or(PresentationLayoutError::ArithmeticOverflow)
    })?;
    if full_width <= available_width {
        return Ok(MeasuredTextFit {
            visible_characters: character_advances.len(),
            elided: false,
            measured_width: full_width,
        });
    }
    let content_width = available_width.saturating_sub(u32::from(ellipsis_width));
    let mut visible_characters = 0;
    let mut visible_width = 0u32;
    for width in character_advances {
        let next = visible_width
            .checked_add(u32::from(*width))
            .ok_or(PresentationLayoutError::ArithmeticOverflow)?;
        if next > content_width {
            break;
        }
        visible_width = next;
        visible_characters += 1;
    }
    Ok(MeasuredTextFit {
        visible_characters,
        elided: true,
        measured_width: visible_width.saturating_add(u32::from(ellipsis_width)),
    })
}

fn region(
    id: PresentationRegionId,
    bounds: LayoutRect,
    priority: PresentationPriority,
    overflow: PresentationOverflow,
) -> PresentationRegion {
    PresentationRegion {
        id,
        bounds,
        priority,
        overflow,
        mode: PresentationRegionMode::Allocated,
    }
}

fn contains(parent: LayoutRect, child: LayoutRect) -> bool {
    let parent_right = i32::from(parent.x) + i32::from(parent.width);
    let parent_bottom = i32::from(parent.y) + i32::from(parent.height);
    i32::from(child.x) >= i32::from(parent.x)
        && i32::from(child.y) >= i32::from(parent.y)
        && i32::from(child.x) + i32::from(child.width) <= parent_right
        && i32::from(child.y) + i32::from(child.height) <= parent_bottom
}

fn intersects(first: LayoutRect, second: LayoutRect) -> bool {
    i32::from(first.x) < i32::from(second.x) + i32::from(second.width)
        && i32::from(second.x) < i32::from(first.x) + i32::from(first.width)
        && i32::from(first.y) < i32::from(second.y) + i32::from(second.height)
        && i32::from(second.y) < i32::from(first.y) + i32::from(first.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_unselected_surface_gives_prime_space_to_the_canvas() {
        let layout = ResponsivePatchbayLayout::allocate(1366, 768, 100, false).unwrap();
        assert!(layout.region(PresentationRegionId::Inspector).is_none());
        assert_eq!(
            layout
                .region(PresentationRegionId::Canvas)
                .unwrap()
                .bounds
                .width,
            1190
        );
        layout.validate().unwrap();
    }

    #[test]
    fn selected_inspector_allocates_or_becomes_an_explicit_drawer() {
        let desktop = ResponsivePatchbayLayout::allocate(1366, 768, 100, true).unwrap();
        assert_eq!(
            desktop
                .region(PresentationRegionId::Inspector)
                .unwrap()
                .mode,
            PresentationRegionMode::Allocated
        );
        let narrow = ResponsivePatchbayLayout::allocate(720, 640, 100, true).unwrap();
        assert_eq!(
            narrow.region(PresentationRegionId::Inspector).unwrap().mode,
            PresentationRegionMode::InspectorDrawer
        );
        narrow.validate().unwrap();
    }

    #[test]
    fn enlarged_text_reallocates_finite_regions_without_collision() {
        let layout = ResponsivePatchbayLayout::allocate(1366, 768, 200, true).unwrap();
        assert!(
            layout
                .region(PresentationRegionId::HeaderMeaning)
                .unwrap()
                .bounds
                .height
                >= 104
        );
        assert_eq!(layout.regions().len(), MAX_PRESENTATION_REGIONS);
        layout.validate().unwrap();
    }

    #[test]
    fn measured_text_elides_on_character_boundaries() {
        let text = "Body 机 — 0123456789";
        let advances = text
            .chars()
            .map(|character| if character == '机' { 16 } else { 8 })
            .collect::<Vec<_>>();
        let fit = fit_measured_text(text, &advances, 72, 8).unwrap();
        assert!(fit.elided);
        assert_eq!(fit.visible_characters, 7);
        assert!(fit.measured_width <= 72);
        assert!(text.chars().count() > fit.visible_characters);
    }

    #[test]
    fn invalid_viewport_scale_and_measurement_fail_deterministically() {
        assert_eq!(
            ResponsivePatchbayLayout::allocate(479, 768, 100, false),
            Err(PresentationLayoutError::UnsupportedViewport)
        );
        assert_eq!(
            ResponsivePatchbayLayout::allocate(1366, 768, 201, false),
            Err(PresentationLayoutError::InvalidTextScale)
        );
        assert_eq!(
            fit_measured_text("two", &[8, 8], 32, 8),
            Err(PresentationLayoutError::InvalidMeasurement)
        );
    }
}
