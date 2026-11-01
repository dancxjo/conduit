//! Finite straight-segment geometry; no interpolation or temporal meaning.
use super::{validate_rect, GraphicsError, LayoutRect};
use crate::GraphicsPoint;

pub const MAX_GRAPHICS_POLYLINE_POINTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsPolyline {
    points: [GraphicsPoint; MAX_GRAPHICS_POLYLINE_POINTS],
    count: u8,
}

impl GraphicsPolyline {
    pub fn new(points: &[GraphicsPoint]) -> Result<Self, GraphicsError> {
        if !(2..=MAX_GRAPHICS_POLYLINE_POINTS).contains(&points.len()) {
            return Err(GraphicsError::InvalidGeometry);
        }
        if points.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(GraphicsError::InvalidGeometry);
        }
        let mut stored = [GraphicsPoint::default(); MAX_GRAPHICS_POLYLINE_POINTS];
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
        let (mut left, mut right, mut top, mut bottom) =
            (*first.x(), *first.x(), *first.y(), *first.y());
        for point in self.points() {
            left = left.min(*point.x());
            right = right.max(*point.x());
            top = top.min(*point.y());
            bottom = bottom.max(*point.y());
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

    pub(super) fn encode(self) -> ([u8; MAX_GRAPHICS_POLYLINE_POINTS * 4], usize) {
        let mut bytes = [0; MAX_GRAPHICS_POLYLINE_POINTS * 4];
        for (point, output) in self
            .points()
            .iter()
            .zip(bytes.as_chunks_mut::<4>().0.iter_mut())
        {
            output[..2].copy_from_slice(&point.x().to_le_bytes());
            output[2..].copy_from_slice(&point.y().to_le_bytes());
        }
        (bytes, usize::from(self.count) * 4)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, GraphicsError> {
        if !bytes.len().is_multiple_of(4)
            || !(8..=MAX_GRAPHICS_POLYLINE_POINTS * 4).contains(&bytes.len())
        {
            return Err(GraphicsError::MalformedEncoding);
        }
        let mut points = [GraphicsPoint::default(); MAX_GRAPHICS_POLYLINE_POINTS];
        for (input, point) in bytes.as_chunks::<4>().0.iter().zip(points.iter_mut()) {
            *point = GraphicsPoint::new(
                i16::from_le_bytes([input[0], input[1]]),
                i16::from_le_bytes([input[2], input[3]]),
            )
            .map_err(|_| GraphicsError::InvalidGeometry)?;
        }
        Self::new(&points[..bytes.len() / 4])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphicsCommand, GraphicsPaintRole, GraphicsScene};
    #[test]
    fn diagonal_roundtrip_preserves_exact_points_and_command_bound() {
        let points = [
            GraphicsPoint::new(1, 2).unwrap(),
            GraphicsPoint::new(9, 13).unwrap(),
            GraphicsPoint::new(12, 3).unwrap(),
        ];
        let line = GraphicsPolyline::new(&points).unwrap();
        let clip = LayoutRect {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(GraphicsCommand::polyline(line, clip, GraphicsPaintRole::Foreground).unwrap())
            .unwrap();
        let bytes = scene.encode();
        let n = scene.encoded_len();
        let decoded = GraphicsScene::decode(&bytes[..n]).unwrap();
        assert_eq!(
            decoded.commands()[0].polyline_geometry().unwrap().points(),
            &points
        );
        assert_eq!(decoded.commands()[0].payload(), "");
        assert!(crate::GraphicsPath::new(&points).is_err());
    }
    #[test]
    fn duplicate_or_unbounded_point_collections_refuse() {
        let point = GraphicsPoint::new(1, 1).unwrap();
        assert!(GraphicsPolyline::new(&[]).is_err());
        assert!(GraphicsPolyline::new(&[point]).is_err());
        assert!(GraphicsPolyline::new(&[point, point]).is_err());
        assert!(GraphicsPolyline::new(&[point; MAX_GRAPHICS_POLYLINE_POINTS + 1]).is_err());
    }
}
