//! Pre-admitted workspace for a continuous local Vision realization.

use crate::{
    LocalCvRefusal, MotionRegion, PixelRegion, MAXIMUM_LOCAL_COMPONENTS, MAXIMUM_LOCAL_CV_HEIGHT,
    MAXIMUM_LOCAL_CV_PIXELS, MAXIMUM_LOCAL_CV_WIDTH,
};
use alloc::{collections::VecDeque, vec, vec::Vec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuousLocalVisionStorage {
    pub previous_pixels: usize,
    pub visited_pixels: usize,
    pub queued_pixels: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuousLocalVisionObservation {
    pub sequence: u64,
    pub motion: Option<MotionRegion>,
    pub components: [Option<PixelRegion>; MAXIMUM_LOCAL_COMPONENTS],
    pub component_areas: [u32; MAXIMUM_LOCAL_COMPONENTS],
    pub component_count: u8,
    pub observed_component_count: u32,
    pub components_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuousLocalVisionRefusal {
    LocalCv(LocalCvRefusal),
    SequenceExhausted,
}

/// Reusable finite scratch for an externally unbounded sequence of frames.
/// Construction is the only point at which storage grows.
pub struct ContinuousLocalVision {
    width: usize,
    height: usize,
    maximum_components: usize,
    previous: Vec<u8>,
    visited: Vec<bool>,
    queue: VecDeque<usize>,
    has_previous: bool,
    sequence: u64,
}

impl ContinuousLocalVision {
    pub fn new(
        width: u16,
        height: u16,
        maximum_components: usize,
    ) -> Result<Self, ContinuousLocalVisionRefusal> {
        if width == 0 || height == 0 {
            return Err(ContinuousLocalVisionRefusal::LocalCv(
                LocalCvRefusal::EmptyFrame,
            ));
        }
        if width > MAXIMUM_LOCAL_CV_WIDTH || height > MAXIMUM_LOCAL_CV_HEIGHT {
            return Err(ContinuousLocalVisionRefusal::LocalCv(
                LocalCvRefusal::DimensionBound,
            ));
        }
        if maximum_components == 0 || maximum_components > MAXIMUM_LOCAL_COMPONENTS {
            return Err(ContinuousLocalVisionRefusal::LocalCv(
                LocalCvRefusal::InvalidComponentBound,
            ));
        }
        let width = usize::from(width);
        let height = usize::from(height);
        let pixels = width
            .checked_mul(height)
            .filter(|count| *count <= MAXIMUM_LOCAL_CV_PIXELS)
            .ok_or(ContinuousLocalVisionRefusal::LocalCv(
                LocalCvRefusal::DimensionBound,
            ))?;
        Ok(Self {
            width,
            height,
            maximum_components,
            previous: vec![0; pixels],
            visited: vec![false; pixels],
            queue: VecDeque::with_capacity(pixels),
            has_previous: false,
            sequence: 0,
        })
    }

    pub fn storage(&self) -> ContinuousLocalVisionStorage {
        ContinuousLocalVisionStorage {
            previous_pixels: self.previous.capacity(),
            visited_pixels: self.visited.capacity(),
            queued_pixels: self.queue.capacity(),
        }
    }

    pub fn observe(
        &mut self,
        pixels: &[u8],
        minimum_motion_delta: u8,
        component_threshold: u8,
        minimum_component_area: u32,
    ) -> Result<ContinuousLocalVisionObservation, ContinuousLocalVisionRefusal> {
        if pixels.len() != self.previous.len() {
            return Err(ContinuousLocalVisionRefusal::LocalCv(
                LocalCvRefusal::PixelCountMismatch,
            ));
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(ContinuousLocalVisionRefusal::SequenceExhausted)?;
        let motion = self
            .has_previous
            .then(|| motion(self.width, &self.previous, pixels, minimum_motion_delta))
            .flatten();
        let (components, component_areas, component_count, observed_component_count) =
            self.components(pixels, component_threshold, minimum_component_area);
        self.previous.copy_from_slice(pixels);
        self.has_previous = true;
        self.sequence = sequence;
        Ok(ContinuousLocalVisionObservation {
            sequence,
            motion,
            components,
            component_areas,
            component_count: component_count as u8,
            observed_component_count,
            components_truncated: observed_component_count > component_count as u32,
        })
    }

    fn components(
        &mut self,
        pixels: &[u8],
        threshold: u8,
        minimum_area: u32,
    ) -> (
        [Option<PixelRegion>; MAXIMUM_LOCAL_COMPONENTS],
        [u32; MAXIMUM_LOCAL_COMPONENTS],
        usize,
        u32,
    ) {
        self.visited.fill(false);
        self.queue.clear();
        let mut output = [None; MAXIMUM_LOCAL_COMPONENTS];
        let mut areas = [0; MAXIMUM_LOCAL_COMPONENTS];
        let mut count = 0usize;
        let mut observed = 0u32;
        for start in 0..pixels.len() {
            if self.visited[start] || pixels[start] < threshold {
                continue;
            }
            self.visited[start] = true;
            self.queue.push_back(start);
            let mut area = 0u32;
            let mut min_x = self.width;
            let mut min_y = self.height;
            let mut max_x = 0usize;
            let mut max_y = 0usize;
            while let Some(index) = self.queue.pop_front() {
                area = area.saturating_add(1);
                let x = index % self.width;
                let y = index / self.width;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                for neighbor in neighbors(x, y, self.width, self.height)
                    .into_iter()
                    .flatten()
                {
                    if !self.visited[neighbor] && pixels[neighbor] >= threshold {
                        self.visited[neighbor] = true;
                        self.queue.push_back(neighbor);
                    }
                }
            }
            if area >= minimum_area {
                observed = observed.saturating_add(1);
                if count < self.maximum_components {
                    output[count] = Some(PixelRegion {
                        x: min_x as u16,
                        y: min_y as u16,
                        width: (max_x - min_x + 1) as u16,
                        height: (max_y - min_y + 1) as u16,
                    });
                    areas[count] = area;
                    count += 1;
                }
            }
        }
        (output, areas, count, observed)
    }
}

fn motion(width: usize, previous: &[u8], current: &[u8], delta: u8) -> Option<MotionRegion> {
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    let mut changed_pixels = 0u32;
    for (index, (&before, &after)) in previous.iter().zip(current).enumerate() {
        if before.abs_diff(after) < delta {
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
    bounds.map(|(min_x, min_y, max_x, max_y)| MotionRegion {
        region: PixelRegion {
            x: min_x as u16,
            y: min_y as u16,
            width: (max_x - min_x + 1) as u16,
            height: (max_y - min_y + 1) as u16,
        },
        changed_pixels,
    })
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
    fn one_workspace_crosses_one_hundred_thousand_frames_without_growth_or_restart() {
        let mut vision = ContinuousLocalVision::new(4, 4, 4).unwrap();
        let admitted = vision.storage();
        let dark = [0u8; 16];
        let mut bright = [0u8; 16];
        bright[5] = 255;
        bright[6] = 255;
        let mut final_observation = None;
        for index in 0..100_000 {
            final_observation = Some(
                vision
                    .observe(if index % 2 == 0 { &dark } else { &bright }, 32, 128, 2)
                    .unwrap(),
            );
            assert_eq!(vision.storage(), admitted);
        }
        let final_observation = final_observation.unwrap();
        assert_eq!(final_observation.sequence, 100_000);
        assert_eq!(final_observation.motion.unwrap().changed_pixels, 2);
        assert_eq!(final_observation.component_count, 1);
        assert_eq!(final_observation.component_areas[0], 2);
        assert_eq!(final_observation.observed_component_count, 1);
        assert!(!final_observation.components_truncated);
    }

    #[test]
    fn component_pressure_is_explicit_instead_of_silent() {
        let mut vision = ContinuousLocalVision::new(5, 2, 2).unwrap();
        let pixels = [255, 0, 255, 0, 255, 0, 255, 0, 255, 0];
        let observation = vision.observe(&pixels, 32, 128, 1).unwrap();
        assert_eq!(observation.component_count, 2);
        assert_eq!(&observation.component_areas[..2], &[1, 1]);
        assert_eq!(observation.observed_component_count, 5);
        assert!(observation.components_truncated);
    }
}
