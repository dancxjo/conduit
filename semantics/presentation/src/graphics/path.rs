//! Finite orthogonal geometry; semantic Cord identity remains above this leaf.
use super::{validate_rect, GraphicsError, LayoutRect};

pub const MAX_GRAPHICS_PATH_POINTS: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphicsPoint {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsPath {
    points: [GraphicsPoint; MAX_GRAPHICS_PATH_POINTS],
    count: u8,
}

impl GraphicsPath {
    pub fn new(points: &[GraphicsPoint]) -> Result<Self, GraphicsError> {
        if !(2..=MAX_GRAPHICS_PATH_POINTS).contains(&points.len()) {
            return Err(GraphicsError::InvalidGeometry);
        }
        if points
            .windows(2)
            .any(|pair| pair[0] == pair[1] || (pair[0].x != pair[1].x && pair[0].y != pair[1].y))
        {
            return Err(GraphicsError::InvalidGeometry);
        }
        let mut stored = [GraphicsPoint::default(); MAX_GRAPHICS_PATH_POINTS];
        stored[..points.len()].copy_from_slice(points);
        let path = Self {
            points: stored,
            count: points.len() as u8,
        };
        path.bounds()?;
        Ok(path)
    }

    pub fn points(&self) -> &[GraphicsPoint] {
        &self.points[..usize::from(self.count)]
    }

    pub fn bounds(&self) -> Result<LayoutRect, GraphicsError> {
        let first = self.points[0];
        let (mut left, mut right, mut top, mut bottom) = (first.x, first.x, first.y, first.y);
        for point in self.points() {
            left = left.min(point.x);
            right = right.max(point.x);
            top = top.min(point.y);
            bottom = bottom.max(point.y);
        }
        let bounds = LayoutRect {
            x: left,
            y: top,
            width: u16::try_from(i32::from(right) - i32::from(left) + 1)
                .map_err(|_| GraphicsError::InvalidGeometry)?,
            height: u16::try_from(i32::from(bottom) - i32::from(top) + 1)
                .map_err(|_| GraphicsError::InvalidGeometry)?,
        };
        validate_rect(bounds)?;
        Ok(bounds)
    }

    pub(super) fn encode(self) -> ([u8; MAX_GRAPHICS_PATH_POINTS * 4], usize) {
        let mut bytes = [0; MAX_GRAPHICS_PATH_POINTS * 4];
        for (point, output) in self
            .points()
            .iter()
            .zip(bytes.as_chunks_mut::<4>().0.iter_mut())
        {
            output[..2].copy_from_slice(&point.x.to_le_bytes());
            output[2..].copy_from_slice(&point.y.to_le_bytes());
        }
        (bytes, usize::from(self.count) * 4)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, GraphicsError> {
        if !bytes.len().is_multiple_of(4)
            || !(8..=MAX_GRAPHICS_PATH_POINTS * 4).contains(&bytes.len())
        {
            return Err(GraphicsError::MalformedEncoding);
        }
        let mut points = [GraphicsPoint::default(); MAX_GRAPHICS_PATH_POINTS];
        for (input, point) in bytes.as_chunks::<4>().0.iter().zip(points.iter_mut()) {
            *point = GraphicsPoint {
                x: i16::from_le_bytes([input[0], input[1]]),
                y: i16::from_le_bytes([input[2], input[3]]),
            };
        }
        Self::new(&points[..bytes.len() / 4])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphicsCommand, GraphicsPaintRole, GraphicsScene};

    #[test]
    fn a_path_round_trips_as_one_bounded_command() {
        let path = GraphicsPath::new(&[
            GraphicsPoint { x: -4, y: 4 },
            GraphicsPoint { x: 10, y: 4 },
            GraphicsPoint { x: 10, y: 20 },
            GraphicsPoint { x: 24, y: 20 },
        ])
        .unwrap();
        let clip = LayoutRect {
            x: 0,
            y: 0,
            width: 32,
            height: 24,
        };
        let command = GraphicsCommand::path(path, clip, GraphicsPaintRole::Accent).unwrap();
        assert_eq!(command.path_geometry(), Some(path));
        assert_eq!(command.payload(), "");
        let mut scene = GraphicsScene::empty();
        scene.push(command).unwrap();
        assert_eq!(scene.commands().len(), 1);
        assert_eq!(scene.encoded_len(), 39);
        let mut encoded = scene.encode();
        assert_eq!(
            GraphicsScene::decode(&encoded[..scene.encoded_len()]),
            Ok(scene)
        );
        encoded[5] = 0;
        assert_eq!(
            GraphicsScene::decode(&encoded[..scene.encoded_len()]),
            Err(GraphicsError::NonCanonicalEncoding)
        );
    }

    #[test]
    fn malformed_diagonal_degenerate_and_excess_geometry_refuse() {
        let a = GraphicsPoint { x: 0, y: 0 };
        assert!(GraphicsPath::new(&[]).is_err());
        assert!(GraphicsPath::new(&[a]).is_err());
        assert!(GraphicsPath::new(&[a, a]).is_err());
        assert!(GraphicsPath::new(&[a, GraphicsPoint { x: 1, y: 1 }]).is_err());
        assert!(GraphicsPath::new(&[a; MAX_GRAPHICS_PATH_POINTS + 1]).is_err());
        assert!(GraphicsPath::new(&[a, GraphicsPoint { x: i16::MAX, y: 0 }]).is_err());
        assert!(GraphicsPath::decode(&[0; 9]).is_err());
        assert!(GraphicsPath::decode(&[0; 36]).is_err());
    }
}
