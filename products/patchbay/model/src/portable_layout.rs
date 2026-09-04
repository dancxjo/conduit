//! Direct presenter-side evaluator for normalized portable layout frames.
//!
//! This intentionally does not call the reference algebra's operations. It is
//! the materially different eager evaluator a presenter can use after decoding
//! the same finite Info contract.

use conduit_presentation::{
    LayoutAlignment, LayoutError, LayoutFrame, LayoutRect, MAX_LAYOUT_CHILDREN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectLayoutOperation {
    Inset(u16),
    Row(u16),
    Column(u16),
    Stack,
    Align {
        horizontal: LayoutAlignment,
        vertical: LayoutAlignment,
    },
}

pub struct DirectLayoutEvaluator;
impl DirectLayoutEvaluator {
    pub fn evaluate(
        mut frame: LayoutFrame,
        operation: DirectLayoutOperation,
    ) -> Result<LayoutFrame, LayoutError> {
        match operation {
            DirectLayoutOperation::Inset(amount) => {
                let size = amount
                    .checked_mul(2)
                    .ok_or(LayoutError::CoordinateOverflow)?;
                frame.viewport.x = frame
                    .viewport
                    .x
                    .checked_add(
                        i16::try_from(amount).map_err(|_| LayoutError::CoordinateOverflow)?,
                    )
                    .ok_or(LayoutError::CoordinateOverflow)?;
                frame.viewport.y = frame
                    .viewport
                    .y
                    .checked_add(
                        i16::try_from(amount).map_err(|_| LayoutError::CoordinateOverflow)?,
                    )
                    .ok_or(LayoutError::CoordinateOverflow)?;
                frame.viewport.width = frame
                    .viewport
                    .width
                    .checked_sub(size)
                    .ok_or(LayoutError::UndersizedExtent)?;
                frame.viewport.height = frame
                    .viewport
                    .height
                    .checked_sub(size)
                    .ok_or(LayoutError::UndersizedExtent)?;
                for child in frame.children.iter_mut().take(frame.child_count as usize) {
                    *child = clip(*child, frame.viewport)?;
                }
            }
            DirectLayoutOperation::Row(gap) => distribute(&mut frame, true, gap)?,
            DirectLayoutOperation::Column(gap) => distribute(&mut frame, false, gap)?,
            DirectLayoutOperation::Stack => {
                for child in frame.children.iter_mut().take(frame.child_count as usize) {
                    *child = frame.viewport;
                }
            }
            DirectLayoutOperation::Align {
                horizontal,
                vertical,
            } => {
                for child in frame.children.iter_mut().take(frame.child_count as usize) {
                    child.width = child.width.min(frame.viewport.width);
                    child.height = child.height.min(frame.viewport.height);
                    child.x = align(
                        frame.viewport.x,
                        frame.viewport.width,
                        child.width,
                        horizontal,
                    )?;
                    child.y = align(
                        frame.viewport.y,
                        frame.viewport.height,
                        child.height,
                        vertical,
                    )?;
                }
            }
        }
        Ok(frame)
    }
}

fn distribute(frame: &mut LayoutFrame, horizontal: bool, gap: u16) -> Result<(), LayoutError> {
    let count = u16::from(frame.child_count);
    if count == 0 {
        return Ok(());
    }
    if frame.child_count as usize > MAX_LAYOUT_CHILDREN {
        return Err(LayoutError::TooManyChildren);
    }
    let total_gap = gap
        .checked_mul(count - 1)
        .ok_or(LayoutError::CoordinateOverflow)?;
    let extent = if horizontal {
        frame.viewport.width
    } else {
        frame.viewport.height
    };
    let usable = extent
        .checked_sub(total_gap)
        .ok_or(LayoutError::UndersizedExtent)?;
    let small = usable / count;
    let large_count = usable % count;
    let mut cursor = 0u16;
    for index in 0..count as usize {
        let size = small + u16::from((index as u16) < large_count);
        let offset = i16::try_from(cursor).map_err(|_| LayoutError::CoordinateOverflow)?;
        frame.children[index] = if horizontal {
            LayoutRect {
                x: frame
                    .viewport
                    .x
                    .checked_add(offset)
                    .ok_or(LayoutError::CoordinateOverflow)?,
                y: frame.viewport.y,
                width: size,
                height: frame.viewport.height,
            }
        } else {
            LayoutRect {
                x: frame.viewport.x,
                y: frame
                    .viewport
                    .y
                    .checked_add(offset)
                    .ok_or(LayoutError::CoordinateOverflow)?,
                width: frame.viewport.width,
                height: size,
            }
        };
        cursor = cursor
            .checked_add(size)
            .and_then(|value| {
                if index + 1 < count as usize {
                    value.checked_add(gap)
                } else {
                    Some(value)
                }
            })
            .ok_or(LayoutError::CoordinateOverflow)?;
    }
    Ok(())
}
fn align(
    origin: i16,
    available: u16,
    child: u16,
    alignment: LayoutAlignment,
) -> Result<i16, LayoutError> {
    let free = available - child;
    let offset = match alignment {
        LayoutAlignment::Start => 0,
        LayoutAlignment::Center => free / 2,
        LayoutAlignment::End => free,
    };
    origin
        .checked_add(i16::try_from(offset).map_err(|_| LayoutError::CoordinateOverflow)?)
        .ok_or(LayoutError::CoordinateOverflow)
}
fn clip(child: LayoutRect, viewport: LayoutRect) -> Result<LayoutRect, LayoutError> {
    let x = i32::from(child.x).max(i32::from(viewport.x));
    let y = i32::from(child.y).max(i32::from(viewport.y));
    let end_x = (i32::from(child.x) + i32::from(child.width))
        .min(i32::from(viewport.x) + i32::from(viewport.width))
        .max(x);
    let end_y = (i32::from(child.y) + i32::from(child.height))
        .min(i32::from(viewport.y) + i32::from(viewport.height))
        .max(y);
    Ok(LayoutRect {
        x: i16::try_from(x).map_err(|_| LayoutError::CoordinateOverflow)?,
        y: i16::try_from(y).map_err(|_| LayoutError::CoordinateOverflow)?,
        width: u16::try_from(end_x - x).map_err(|_| LayoutError::CoordinateOverflow)?,
        height: u16::try_from(end_y - y).map_err(|_| LayoutError::CoordinateOverflow)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::{LayoutAxis, MAX_LAYOUT_CHILDREN};
    #[test]
    fn direct_presenter_matches_normalized_reference_vectors() {
        let cases = [
            LayoutFrame::viewport(10, 4, 3, 2, 2).unwrap(),
            LayoutFrame::viewport(32, 16, MAX_LAYOUT_CHILDREN as u8, 4, 3).unwrap(),
            LayoutFrame::viewport(8, 8, 0, 0, 0).unwrap(),
        ];
        for frame in cases {
            assert_eq!(
                DirectLayoutEvaluator::evaluate(frame, DirectLayoutOperation::Row(1)),
                frame.distribute(LayoutAxis::Horizontal, 1)
            );
            assert_eq!(
                DirectLayoutEvaluator::evaluate(frame, DirectLayoutOperation::Column(1)),
                frame.distribute(LayoutAxis::Vertical, 1)
            );
            assert_eq!(
                DirectLayoutEvaluator::evaluate(frame, DirectLayoutOperation::Stack),
                Ok(frame.stack())
            );
        }
        let frame = LayoutFrame::viewport(20, 12, 2, 4, 3).unwrap();
        assert_eq!(
            DirectLayoutEvaluator::evaluate(frame, DirectLayoutOperation::Inset(2)),
            frame.inset(2)
        );
        assert_eq!(
            DirectLayoutEvaluator::evaluate(
                frame,
                DirectLayoutOperation::Align {
                    horizontal: LayoutAlignment::Center,
                    vertical: LayoutAlignment::End
                }
            ),
            frame.align(LayoutAlignment::Center, LayoutAlignment::End)
        );
        let undersized = LayoutFrame::viewport(3, 3, 3, 1, 1).unwrap();
        assert_eq!(
            DirectLayoutEvaluator::evaluate(undersized, DirectLayoutOperation::Row(2)),
            Err(LayoutError::UndersizedExtent)
        );
    }
}
