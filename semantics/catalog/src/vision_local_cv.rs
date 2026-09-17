//! Bounded local-CV primitives for exact Vision provider implementations.
//!
//! These algorithms operate only on pixels already admitted by an image Base.
//! They acquire no camera and carry no authored placement or provider identity.

use alloc::{collections::VecDeque, vec, vec::Vec};

pub const MAXIMUM_LOCAL_CV_WIDTH: u16 = 1_280;
pub const MAXIMUM_LOCAL_CV_HEIGHT: u16 = 720;
pub const MAXIMUM_LOCAL_CV_PIXELS: usize =
    MAXIMUM_LOCAL_CV_WIDTH as usize * MAXIMUM_LOCAL_CV_HEIGHT as usize;
pub const MAXIMUM_LOCAL_COMPONENTS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelRegion {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionRegion {
    pub region: PixelRegion,
    pub changed_pixels: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrayFrame {
    width: u16,
    height: u16,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalCvRefusal {
    EmptyFrame,
    DimensionBound,
    PixelCountMismatch,
    FrameShapeMismatch,
    InvalidComponentBound,
}

impl GrayFrame {
    pub fn new(width: u16, height: u16, pixels: Vec<u8>) -> Result<Self, LocalCvRefusal> {
        if width == 0 || height == 0 {
            return Err(LocalCvRefusal::EmptyFrame);
        }
        if width > MAXIMUM_LOCAL_CV_WIDTH || height > MAXIMUM_LOCAL_CV_HEIGHT {
            return Err(LocalCvRefusal::DimensionBound);
        }
        let expected = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or(LocalCvRefusal::DimensionBound)?;
        if expected > MAXIMUM_LOCAL_CV_PIXELS {
            return Err(LocalCvRefusal::DimensionBound);
        }
        if pixels.len() != expected {
            return Err(LocalCvRefusal::PixelCountMismatch);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// Absolute frame differencing with one bounded resulting region.
pub fn detect_motion(
    previous: &GrayFrame,
    current: &GrayFrame,
    minimum_delta: u8,
) -> Result<Option<MotionRegion>, LocalCvRefusal> {
    if previous.width != current.width || previous.height != current.height {
        return Err(LocalCvRefusal::FrameShapeMismatch);
    }
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    let mut changed_pixels = 0u32;
    let width = usize::from(current.width);
    for (index, (&before, &after)) in previous
        .pixels
        .iter()
        .zip(current.pixels.iter())
        .enumerate()
    {
        if before.abs_diff(after) < minimum_delta {
            continue;
        }
        changed_pixels = changed_pixels.saturating_add(1);
        let x = index % width;
        let y = index / width;
        bounds = Some(match bounds {
            None => (x, y, x, y),
            Some((min_x, min_y, max_x, max_y)) => {
                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
            }
        });
    }
    let Some((min_x, min_y, max_x, max_y)) = bounds else {
        return Ok(None);
    };
    Ok(Some(MotionRegion {
        region: PixelRegion {
            x: min_x as u16,
            y: min_y as u16,
            width: (max_x - min_x + 1) as u16,
            height: (max_y - min_y + 1) as u16,
        },
        changed_pixels,
    }))
}

/// Threshold + four-connected component extraction, matching the ordinary
/// local-CV contour/region seam without depending on one library ABI.
pub fn detect_bright_components(
    frame: &GrayFrame,
    threshold: u8,
    minimum_area: u32,
    maximum_components: usize,
) -> Result<Vec<PixelRegion>, LocalCvRefusal> {
    if maximum_components == 0 || maximum_components > MAXIMUM_LOCAL_COMPONENTS {
        return Err(LocalCvRefusal::InvalidComponentBound);
    }
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let mut visited = vec![false; frame.pixels.len()];
    let mut regions = Vec::with_capacity(maximum_components);
    let mut queue = VecDeque::with_capacity(frame.pixels.len().min(4_096));

    for start in 0..frame.pixels.len() {
        if visited[start] || frame.pixels[start] < threshold {
            continue;
        }
        visited[start] = true;
        queue.push_back(start);
        let mut area = 0u32;
        let mut min_x = width;
        let mut min_y = height;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        while let Some(index) = queue.pop_front() {
            area = area.saturating_add(1);
            let x = index % width;
            let y = index / width;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            for neighbor in neighbors(x, y, width, height).into_iter().flatten() {
                if !visited[neighbor] && frame.pixels[neighbor] >= threshold {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        if area >= minimum_area && regions.len() < maximum_components {
            regions.push(PixelRegion {
                x: min_x as u16,
                y: min_y as u16,
                width: (max_x - min_x + 1) as u16,
                height: (max_y - min_y + 1) as u16,
            });
        }
    }
    Ok(regions)
}

fn neighbors(x: usize, y: usize, width: usize, height: usize) -> [Option<usize>; 4] {
    [
        x.checked_sub(1).map(|next| y * width + next),
        (x + 1 < width).then_some(y * width + x + 1),
        y.checked_sub(1).map(|next| next * width + x),
        (y + 1 < height).then_some((y + 1) * width + x),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_difference_retains_exact_changed_extent() {
        let before = GrayFrame::new(4, 3, vec![0; 12]).unwrap();
        let mut pixels = vec![0; 12];
        pixels[5] = 200;
        pixels[6] = 220;
        let after = GrayFrame::new(4, 3, pixels).unwrap();
        assert_eq!(
            detect_motion(&before, &after, 32).unwrap(),
            Some(MotionRegion {
                region: PixelRegion {
                    x: 1,
                    y: 1,
                    width: 2,
                    height: 1,
                },
                changed_pixels: 2,
            })
        );
    }

    #[test]
    fn connected_components_are_bounded_and_spatially_exact() {
        let frame = GrayFrame::new(
            5,
            4,
            vec![
                255, 255, 0, 0, 0, 255, 255, 0, 255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0,
            ],
        )
        .unwrap();
        assert_eq!(
            detect_bright_components(&frame, 128, 2, 4).unwrap(),
            vec![
                PixelRegion {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                },
                PixelRegion {
                    x: 3,
                    y: 1,
                    width: 1,
                    height: 2,
                },
            ]
        );
        assert_eq!(
            detect_bright_components(&frame, 128, 1, 17),
            Err(LocalCvRefusal::InvalidComponentBound)
        );
    }
}
