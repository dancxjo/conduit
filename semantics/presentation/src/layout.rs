//! Fixed-capacity portable presentation geometry.

pub const LAYOUT_FRAME_KIND: &str = "presentation/layout-frame@1";
pub const MAX_LAYOUT_CHILDREN: usize = 8;
pub const MAX_LAYOUT_EXTENT: u16 = i16::MAX as u16;
pub const MAX_LAYOUT_FRAME_BYTES: usize = 10 + MAX_LAYOUT_CHILDREN * 8;
const LAYOUT_FRAME_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutRect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutFrame {
    pub viewport: LayoutRect,
    pub child_count: u8,
    pub children: [LayoutRect; MAX_LAYOUT_CHILDREN],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutAlignment {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    TooManyChildren,
    ExtentOutOfBounds,
    UndersizedExtent,
    CoordinateOverflow,
    MalformedEncoding,
    NonCanonicalEncoding,
}

impl LayoutFrame {
    pub fn viewport(
        width: u16,
        height: u16,
        child_count: u8,
        child_width: u16,
        child_height: u16,
    ) -> Result<Self, LayoutError> {
        validate_extent(width, height)?;
        validate_extent(child_width, child_height)?;
        if usize::from(child_count) > MAX_LAYOUT_CHILDREN {
            return Err(LayoutError::TooManyChildren);
        }
        let mut children = [LayoutRect::default(); MAX_LAYOUT_CHILDREN];
        for child in children.iter_mut().take(usize::from(child_count)) {
            *child = LayoutRect {
                x: 0,
                y: 0,
                width: child_width,
                height: child_height,
            };
        }
        Ok(Self {
            viewport: LayoutRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            child_count,
            children,
        })
    }

    pub fn inset(mut self, inset: u16) -> Result<Self, LayoutError> {
        let doubled = inset
            .checked_mul(2)
            .ok_or(LayoutError::CoordinateOverflow)?;
        self.viewport.width = self
            .viewport
            .width
            .checked_sub(doubled)
            .ok_or(LayoutError::UndersizedExtent)?;
        self.viewport.height = self
            .viewport
            .height
            .checked_sub(doubled)
            .ok_or(LayoutError::UndersizedExtent)?;
        let shift = i16::try_from(inset).map_err(|_| LayoutError::CoordinateOverflow)?;
        self.viewport.x = self
            .viewport
            .x
            .checked_add(shift)
            .ok_or(LayoutError::CoordinateOverflow)?;
        self.viewport.y = self
            .viewport
            .y
            .checked_add(shift)
            .ok_or(LayoutError::CoordinateOverflow)?;
        self.clip_children()?;
        Ok(self)
    }

    pub fn distribute(self, axis: LayoutAxis, gap: u16) -> Result<Self, LayoutError> {
        if self.child_count == 0 {
            return Ok(self);
        }
        let count = u16::from(self.child_count);
        let gaps = gap
            .checked_mul(count.saturating_sub(1))
            .ok_or(LayoutError::CoordinateOverflow)?;
        let extent = match axis {
            LayoutAxis::Horizontal => self.viewport.width,
            LayoutAxis::Vertical => self.viewport.height,
        };
        let available = extent
            .checked_sub(gaps)
            .ok_or(LayoutError::UndersizedExtent)?;
        let base = available / count;
        let remainder = available % count;
        let mut output = self;
        let mut offset = 0_u16;
        for index in 0..usize::from(count) {
            let size = base + u16::from((index as u16) < remainder);
            let position = offset
                .checked_add(gap.saturating_mul(index as u16))
                .ok_or(LayoutError::CoordinateOverflow)?;
            let coordinate =
                i16::try_from(position).map_err(|_| LayoutError::CoordinateOverflow)?;
            output.children[index] = match axis {
                LayoutAxis::Horizontal => LayoutRect {
                    x: output
                        .viewport
                        .x
                        .checked_add(coordinate)
                        .ok_or(LayoutError::CoordinateOverflow)?,
                    y: output.viewport.y,
                    width: size,
                    height: output.viewport.height,
                },
                LayoutAxis::Vertical => LayoutRect {
                    x: output.viewport.x,
                    y: output
                        .viewport
                        .y
                        .checked_add(coordinate)
                        .ok_or(LayoutError::CoordinateOverflow)?,
                    width: output.viewport.width,
                    height: size,
                },
            };
            offset = offset
                .checked_add(size)
                .ok_or(LayoutError::CoordinateOverflow)?;
        }
        Ok(output)
    }

    pub fn stack(mut self) -> Self {
        for child in self.children.iter_mut().take(usize::from(self.child_count)) {
            *child = self.viewport;
        }
        self
    }

    pub fn align(
        mut self,
        horizontal: LayoutAlignment,
        vertical: LayoutAlignment,
    ) -> Result<Self, LayoutError> {
        for child in self.children.iter_mut().take(usize::from(self.child_count)) {
            child.width = child.width.min(self.viewport.width);
            child.height = child.height.min(self.viewport.height);
            child.x = aligned_coordinate(
                self.viewport.x,
                self.viewport.width,
                child.width,
                horizontal,
            )?;
            child.y = aligned_coordinate(
                self.viewport.y,
                self.viewport.height,
                child.height,
                vertical,
            )?;
        }
        Ok(self)
    }

    pub fn encode(self) -> [u8; MAX_LAYOUT_FRAME_BYTES] {
        let mut output = [0; MAX_LAYOUT_FRAME_BYTES];
        output[0] = LAYOUT_FRAME_VERSION;
        output[1] = self.child_count;
        write_rect(&mut output[2..10], self.viewport);
        for (index, child) in self
            .children
            .iter()
            .take(usize::from(self.child_count))
            .enumerate()
        {
            let start = 10 + index * 8;
            write_rect(&mut output[start..start + 8], *child);
        }
        output
    }

    pub fn encoded_len(self) -> usize {
        10 + usize::from(self.child_count) * 8
    }

    pub fn decode(input: &[u8]) -> Result<Self, LayoutError> {
        if input.len() < 10 || input[0] != LAYOUT_FRAME_VERSION {
            return Err(LayoutError::MalformedEncoding);
        }
        let child_count = input[1];
        if usize::from(child_count) > MAX_LAYOUT_CHILDREN {
            return Err(LayoutError::TooManyChildren);
        }
        if input.len() != 10 + usize::from(child_count) * 8 {
            return Err(LayoutError::NonCanonicalEncoding);
        }
        let viewport = read_rect(&input[2..10])?;
        validate_rect(viewport)?;
        let mut children = [LayoutRect::default(); MAX_LAYOUT_CHILDREN];
        for (index, child) in children
            .iter_mut()
            .take(usize::from(child_count))
            .enumerate()
        {
            let start = 10 + index * 8;
            *child = read_rect(&input[start..start + 8])?;
            validate_rect(*child)?;
        }
        Ok(Self {
            viewport,
            child_count,
            children,
        })
    }

    fn clip_children(&mut self) -> Result<(), LayoutError> {
        for child in self.children.iter_mut().take(usize::from(self.child_count)) {
            *child = intersection(*child, self.viewport)?;
        }
        Ok(())
    }
}

fn aligned_coordinate(
    origin: i16,
    available: u16,
    child: u16,
    alignment: LayoutAlignment,
) -> Result<i16, LayoutError> {
    let free = available.saturating_sub(child);
    let offset = match alignment {
        LayoutAlignment::Start => 0,
        LayoutAlignment::Center => free / 2,
        LayoutAlignment::End => free,
    };
    origin
        .checked_add(i16::try_from(offset).map_err(|_| LayoutError::CoordinateOverflow)?)
        .ok_or(LayoutError::CoordinateOverflow)
}

fn intersection(left: LayoutRect, right: LayoutRect) -> Result<LayoutRect, LayoutError> {
    let left_end_x = i32::from(left.x) + i32::from(left.width);
    let left_end_y = i32::from(left.y) + i32::from(left.height);
    let right_end_x = i32::from(right.x) + i32::from(right.width);
    let right_end_y = i32::from(right.y) + i32::from(right.height);
    let x = i32::from(left.x).max(i32::from(right.x));
    let y = i32::from(left.y).max(i32::from(right.y));
    let end_x = left_end_x.min(right_end_x).max(x);
    let end_y = left_end_y.min(right_end_y).max(y);
    Ok(LayoutRect {
        x: i16::try_from(x).map_err(|_| LayoutError::CoordinateOverflow)?,
        y: i16::try_from(y).map_err(|_| LayoutError::CoordinateOverflow)?,
        width: u16::try_from(end_x - x).map_err(|_| LayoutError::CoordinateOverflow)?,
        height: u16::try_from(end_y - y).map_err(|_| LayoutError::CoordinateOverflow)?,
    })
}

fn validate_extent(width: u16, height: u16) -> Result<(), LayoutError> {
    if width > MAX_LAYOUT_EXTENT || height > MAX_LAYOUT_EXTENT {
        Err(LayoutError::ExtentOutOfBounds)
    } else {
        Ok(())
    }
}

fn validate_rect(rect: LayoutRect) -> Result<(), LayoutError> {
    validate_extent(rect.width, rect.height)?;
    let end_x = i32::from(rect.x) + i32::from(rect.width);
    let end_y = i32::from(rect.y) + i32::from(rect.height);
    if end_x > i32::from(i16::MAX) || end_y > i32::from(i16::MAX) {
        Err(LayoutError::CoordinateOverflow)
    } else {
        Ok(())
    }
}

fn write_rect(output: &mut [u8], rect: LayoutRect) {
    output[0..2].copy_from_slice(&rect.x.to_le_bytes());
    output[2..4].copy_from_slice(&rect.y.to_le_bytes());
    output[4..6].copy_from_slice(&rect.width.to_le_bytes());
    output[6..8].copy_from_slice(&rect.height.to_le_bytes());
}

fn read_rect(input: &[u8]) -> Result<LayoutRect, LayoutError> {
    if input.len() != 8 {
        return Err(LayoutError::MalformedEncoding);
    }
    Ok(LayoutRect {
        x: i16::from_le_bytes([input[0], input[1]]),
        y: i16::from_le_bytes([input[2], input[3]]),
        width: u16::from_le_bytes([input[4], input[5]]),
        height: u16::from_le_bytes([input[6], input[7]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_rounding_is_stable_and_encoding_is_canonical() {
        let frame = LayoutFrame::viewport(10, 4, 3, 2, 2)
            .unwrap()
            .distribute(LayoutAxis::Horizontal, 1)
            .unwrap();
        assert_eq!(
            frame.children[0],
            LayoutRect {
                x: 0,
                y: 0,
                width: 3,
                height: 4
            }
        );
        assert_eq!(
            frame.children[1],
            LayoutRect {
                x: 4,
                y: 0,
                width: 3,
                height: 4
            }
        );
        assert_eq!(
            frame.children[2],
            LayoutRect {
                x: 8,
                y: 0,
                width: 2,
                height: 4
            }
        );
        let encoded = frame.encode();
        assert_eq!(
            LayoutFrame::decode(&encoded[..frame.encoded_len()]),
            Ok(frame)
        );
        assert_eq!(
            LayoutFrame::decode(&encoded),
            Err(LayoutError::NonCanonicalEncoding)
        );
    }

    #[test]
    fn zero_maximum_undersized_clipping_and_alignment_are_exact() {
        assert_eq!(
            LayoutFrame::viewport(8, 8, 0, 0, 0)
                .unwrap()
                .distribute(LayoutAxis::Vertical, 7)
                .unwrap()
                .child_count,
            0
        );
        let maximum = LayoutFrame::viewport(32, 16, MAX_LAYOUT_CHILDREN as u8, 40, 40)
            .unwrap()
            .inset(2)
            .unwrap();
        assert_eq!(
            maximum.children[0],
            LayoutRect {
                x: 2,
                y: 2,
                width: 28,
                height: 12
            }
        );
        assert_eq!(
            maximum.distribute(LayoutAxis::Horizontal, 5),
            Err(LayoutError::UndersizedExtent)
        );
        let aligned = LayoutFrame::viewport(9, 9, 1, 4, 2)
            .unwrap()
            .align(LayoutAlignment::Center, LayoutAlignment::End)
            .unwrap();
        assert_eq!(
            aligned.children[0],
            LayoutRect {
                x: 2,
                y: 7,
                width: 4,
                height: 2
            }
        );
    }
}
