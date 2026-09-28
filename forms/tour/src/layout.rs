pub const DEFAULT_NARRATIVE_PERCENT: u16 = 46;
pub const DEFAULT_PATCHBAY_PERCENT: u16 = 55;
pub const DEFAULT_SOURCE_PERCENT: u16 = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl TourRect {
    fn right(self) -> Option<u16> {
        self.x.checked_add(self.width)
    }

    fn bottom(self) -> Option<u16> {
        self.y.checked_add(self.height)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourWorkspaceLayout {
    pub viewport: TourRect,
    pub narrative: TourRect,
    pub patchbay: TourRect,
    pub source: TourRect,
    pub output: TourRect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourLayoutRefusal {
    EmptyViewport,
    InvalidSplit,
    ArithmeticOverflow,
    EmptyRegion,
    OutsideViewport,
    OverlappingRegions,
}

impl TourWorkspaceLayout {
    pub fn default_for(width: u16, height: u16) -> Result<Self, TourLayoutRefusal> {
        Self::new(
            width,
            height,
            DEFAULT_NARRATIVE_PERCENT,
            DEFAULT_PATCHBAY_PERCENT,
            DEFAULT_SOURCE_PERCENT,
        )
    }

    pub fn new(
        width: u16,
        height: u16,
        narrative_percent: u16,
        patchbay_percent: u16,
        source_percent: u16,
    ) -> Result<Self, TourLayoutRefusal> {
        if width == 0 || height == 0 {
            return Err(TourLayoutRefusal::EmptyViewport);
        }
        if !(30..=65).contains(&narrative_percent)
            || !(35..=70).contains(&patchbay_percent)
            || !(40..=75).contains(&source_percent)
        {
            return Err(TourLayoutRefusal::InvalidSplit);
        }
        let narrative_width = percent(width, narrative_percent)?;
        let laboratory_width = width
            .checked_sub(narrative_width)
            .ok_or(TourLayoutRefusal::ArithmeticOverflow)?;
        let patchbay_height = percent(height, patchbay_percent)?;
        let lower_height = height
            .checked_sub(patchbay_height)
            .ok_or(TourLayoutRefusal::ArithmeticOverflow)?;
        let source_width = percent(laboratory_width, source_percent)?;
        let output_width = laboratory_width
            .checked_sub(source_width)
            .ok_or(TourLayoutRefusal::ArithmeticOverflow)?;
        let layout = Self {
            viewport: TourRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            narrative: TourRect {
                x: 0,
                y: 0,
                width: narrative_width,
                height,
            },
            patchbay: TourRect {
                x: narrative_width,
                y: 0,
                width: laboratory_width,
                height: patchbay_height,
            },
            source: TourRect {
                x: narrative_width,
                y: patchbay_height,
                width: source_width,
                height: lower_height,
            },
            output: TourRect {
                x: narrative_width
                    .checked_add(source_width)
                    .ok_or(TourLayoutRefusal::ArithmeticOverflow)?,
                y: patchbay_height,
                width: output_width,
                height: lower_height,
            },
        };
        layout.validate()?;
        Ok(layout)
    }

    pub fn validate(&self) -> Result<(), TourLayoutRefusal> {
        let regions = [self.narrative, self.patchbay, self.source, self.output];
        if regions
            .iter()
            .any(|rect| rect.width == 0 || rect.height == 0)
        {
            return Err(TourLayoutRefusal::EmptyRegion);
        }
        let viewport_right = self
            .viewport
            .right()
            .ok_or(TourLayoutRefusal::ArithmeticOverflow)?;
        let viewport_bottom = self
            .viewport
            .bottom()
            .ok_or(TourLayoutRefusal::ArithmeticOverflow)?;
        for rect in regions {
            if rect.x < self.viewport.x
                || rect.y < self.viewport.y
                || rect.right().ok_or(TourLayoutRefusal::ArithmeticOverflow)? > viewport_right
                || rect.bottom().ok_or(TourLayoutRefusal::ArithmeticOverflow)? > viewport_bottom
            {
                return Err(TourLayoutRefusal::OutsideViewport);
            }
        }
        for (index, left) in regions.iter().enumerate() {
            for right in &regions[index + 1..] {
                if overlaps(*left, *right)? {
                    return Err(TourLayoutRefusal::OverlappingRegions);
                }
            }
        }
        Ok(())
    }
}

fn percent(total: u16, value: u16) -> Result<u16, TourLayoutRefusal> {
    let scaled = u32::from(total)
        .checked_mul(u32::from(value))
        .ok_or(TourLayoutRefusal::ArithmeticOverflow)?;
    u16::try_from(scaled / 100).map_err(|_| TourLayoutRefusal::ArithmeticOverflow)
}

fn overlaps(left: TourRect, right: TourRect) -> Result<bool, TourLayoutRefusal> {
    Ok(
        left.x < right.right().ok_or(TourLayoutRefusal::ArithmeticOverflow)?
            && left.right().ok_or(TourLayoutRefusal::ArithmeticOverflow)? > right.x
            && left.y
                < right
                    .bottom()
                    .ok_or(TourLayoutRefusal::ArithmeticOverflow)?
            && left.bottom().ok_or(TourLayoutRefusal::ArithmeticOverflow)? > right.y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_conduitos_workspace_is_contained_and_non_overlapping() {
        let layout = TourWorkspaceLayout::default_for(640, 480).unwrap();
        assert_eq!(
            layout.narrative,
            TourRect {
                x: 0,
                y: 0,
                width: 294,
                height: 480
            }
        );
        assert_eq!(
            layout.patchbay,
            TourRect {
                x: 294,
                y: 0,
                width: 346,
                height: 264
            }
        );
        assert_eq!(
            layout.source,
            TourRect {
                x: 294,
                y: 264,
                width: 207,
                height: 216
            }
        );
        assert_eq!(
            layout.output,
            TourRect {
                x: 501,
                y: 264,
                width: 139,
                height: 216
            }
        );
        assert_eq!(layout.validate(), Ok(()));
    }

    #[test]
    fn malformed_workspace_geometry_is_refused() {
        assert_eq!(
            TourWorkspaceLayout::default_for(0, 480),
            Err(TourLayoutRefusal::EmptyViewport)
        );
        assert_eq!(
            TourWorkspaceLayout::new(640, 480, 29, 55, 60),
            Err(TourLayoutRefusal::InvalidSplit)
        );
        let mut overlapping = TourWorkspaceLayout::default_for(640, 480).unwrap();
        overlapping.output.x = overlapping.source.x;
        assert_eq!(
            overlapping.validate(),
            Err(TourLayoutRefusal::OverlappingRegions)
        );
        let mut outside = TourWorkspaceLayout::default_for(640, 480).unwrap();
        outside.patchbay.width = 500;
        assert_eq!(outside.validate(), Err(TourLayoutRefusal::OutsideViewport));
    }
}
