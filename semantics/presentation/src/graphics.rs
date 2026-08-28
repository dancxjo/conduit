//! Fixed-capacity graphical leaf obligations below semantic presentation.

use crate::{LayoutRect, PresentationIconKey, MAX_LAYOUT_EXTENT};

pub const GRAPHICS_SCENE_KIND: &str = "presentation/graphics-scene@1";
pub const MAX_GRAPHICS_COMMANDS: usize = 8;
pub const MAX_GRAPHICS_TEXT_BYTES: usize = 64;
pub const MAX_GRAPHICS_SCENE_BYTES: usize =
    2 + MAX_GRAPHICS_COMMANDS * (20 + MAX_GRAPHICS_TEXT_BYTES);
const VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphicsCommandKind {
    Rect = 1,
    Text = 2,
    Icon = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphicsPaintRole {
    Background = 1,
    Foreground = 2,
    Accent = 3,
    Status = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphicsShapeStyle {
    Fill = 1,
    Stroke = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsClipClass {
    FullyVisible,
    PartiallyClipped,
    FullyClipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsError {
    EmptyPayload,
    PayloadTooLong,
    TooManyCommands,
    InvalidGeometry,
    UnknownIcon,
    MalformedEncoding,
    NonCanonicalEncoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsCommand {
    pub kind: GraphicsCommandKind,
    pub bounds: LayoutRect,
    pub clip: LayoutRect,
    pub paint: GraphicsPaintRole,
    pub style: GraphicsShapeStyle,
    payload_len: u8,
    payload: [u8; MAX_GRAPHICS_TEXT_BYTES],
}

impl GraphicsCommand {
    pub fn rect(
        bounds: LayoutRect,
        clip: LayoutRect,
        paint: GraphicsPaintRole,
        style: GraphicsShapeStyle,
    ) -> Result<Self, GraphicsError> {
        Self::new(GraphicsCommandKind::Rect, bounds, clip, paint, style, &[])
    }

    pub fn text(
        bounds: LayoutRect,
        clip: LayoutRect,
        paint: GraphicsPaintRole,
        text: &str,
    ) -> Result<Self, GraphicsError> {
        Self::new(
            GraphicsCommandKind::Text,
            bounds,
            clip,
            paint,
            GraphicsShapeStyle::Fill,
            text.as_bytes(),
        )
    }

    pub fn icon(
        bounds: LayoutRect,
        clip: LayoutRect,
        paint: GraphicsPaintRole,
        icon: PresentationIconKey,
    ) -> Result<Self, GraphicsError> {
        Self::new(
            GraphicsCommandKind::Icon,
            bounds,
            clip,
            paint,
            GraphicsShapeStyle::Fill,
            icon.as_str().as_bytes(),
        )
    }

    fn new(
        kind: GraphicsCommandKind,
        bounds: LayoutRect,
        clip: LayoutRect,
        paint: GraphicsPaintRole,
        style: GraphicsShapeStyle,
        payload: &[u8],
    ) -> Result<Self, GraphicsError> {
        validate_rect(bounds)?;
        validate_rect(clip)?;
        if kind != GraphicsCommandKind::Rect && payload.is_empty() {
            return Err(GraphicsError::EmptyPayload);
        }
        if payload.len() > MAX_GRAPHICS_TEXT_BYTES {
            return Err(GraphicsError::PayloadTooLong);
        }
        if kind == GraphicsCommandKind::Icon {
            let token = core::str::from_utf8(payload).map_err(|_| GraphicsError::UnknownIcon)?;
            if PresentationIconKey::from_token(token).is_none() {
                return Err(GraphicsError::UnknownIcon);
            }
        } else if kind == GraphicsCommandKind::Text {
            core::str::from_utf8(payload).map_err(|_| GraphicsError::MalformedEncoding)?;
        } else if !payload.is_empty() {
            return Err(GraphicsError::NonCanonicalEncoding);
        }
        if kind != GraphicsCommandKind::Rect && style != GraphicsShapeStyle::Fill {
            return Err(GraphicsError::NonCanonicalEncoding);
        }
        let mut stored = [0; MAX_GRAPHICS_TEXT_BYTES];
        stored[..payload.len()].copy_from_slice(payload);
        Ok(Self {
            kind,
            bounds,
            clip,
            paint,
            style,
            payload_len: payload.len() as u8,
            payload: stored,
        })
    }

    pub fn payload(&self) -> &str {
        core::str::from_utf8(&self.payload[..usize::from(self.payload_len)])
            .expect("validated graphics payload")
    }

    pub fn clip_class(&self) -> GraphicsClipClass {
        let Some(intersection) = intersection(self.bounds, self.clip) else {
            return GraphicsClipClass::FullyClipped;
        };
        if intersection == self.bounds {
            GraphicsClipClass::FullyVisible
        } else {
            GraphicsClipClass::PartiallyClipped
        }
    }
}

const EMPTY_COMMAND: GraphicsCommand = GraphicsCommand {
    kind: GraphicsCommandKind::Rect,
    bounds: LayoutRect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    },
    clip: LayoutRect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    },
    paint: GraphicsPaintRole::Background,
    style: GraphicsShapeStyle::Fill,
    payload_len: 0,
    payload: [0; MAX_GRAPHICS_TEXT_BYTES],
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsScene {
    count: u8,
    commands: [GraphicsCommand; MAX_GRAPHICS_COMMANDS],
}

impl GraphicsScene {
    pub const fn empty() -> Self {
        Self {
            count: 0,
            commands: [EMPTY_COMMAND; MAX_GRAPHICS_COMMANDS],
        }
    }

    pub fn push(&mut self, command: GraphicsCommand) -> Result<(), GraphicsError> {
        let index = usize::from(self.count);
        if index == MAX_GRAPHICS_COMMANDS {
            return Err(GraphicsError::TooManyCommands);
        }
        self.commands[index] = command;
        self.count += 1;
        Ok(())
    }

    pub fn commands(&self) -> &[GraphicsCommand] {
        &self.commands[..usize::from(self.count)]
    }

    pub fn encode(self) -> [u8; MAX_GRAPHICS_SCENE_BYTES] {
        let mut output = [0; MAX_GRAPHICS_SCENE_BYTES];
        output[0] = VERSION;
        output[1] = self.count;
        let mut offset = 2;
        for command in self.commands() {
            output[offset] = command.kind as u8;
            output[offset + 1] = command.paint as u8;
            output[offset + 2] = command.style as u8;
            write_rect(&mut output[offset + 3..offset + 11], command.bounds);
            write_rect(&mut output[offset + 11..offset + 19], command.clip);
            output[offset + 19] = command.payload_len;
            let len = usize::from(command.payload_len);
            output[offset + 20..offset + 20 + len].copy_from_slice(&command.payload[..len]);
            offset += 20 + len;
        }
        output
    }

    pub fn encoded_len(self) -> usize {
        2 + self
            .commands()
            .iter()
            .map(|command| 20 + usize::from(command.payload_len))
            .sum::<usize>()
    }

    pub fn decode(input: &[u8]) -> Result<Self, GraphicsError> {
        if input.len() < 2 || input[0] != VERSION {
            return Err(GraphicsError::MalformedEncoding);
        }
        let count = usize::from(input[1]);
        if count > MAX_GRAPHICS_COMMANDS {
            return Err(GraphicsError::TooManyCommands);
        }
        let mut scene = Self::empty();
        let mut offset = 2;
        for _ in 0..count {
            if input.len().saturating_sub(offset) < 20 {
                return Err(GraphicsError::MalformedEncoding);
            }
            let kind = decode_kind(input[offset])?;
            let paint = decode_paint(input[offset + 1])?;
            let style = decode_style(input[offset + 2])?;
            let bounds = read_rect(&input[offset + 3..offset + 11]);
            let clip = read_rect(&input[offset + 11..offset + 19]);
            let len = usize::from(input[offset + 19]);
            if len > MAX_GRAPHICS_TEXT_BYTES || input.len().saturating_sub(offset + 20) < len {
                return Err(GraphicsError::PayloadTooLong);
            }
            scene.push(GraphicsCommand::new(
                kind,
                bounds,
                clip,
                paint,
                style,
                &input[offset + 20..offset + 20 + len],
            )?)?;
            offset += 20 + len;
        }
        if offset != input.len() {
            return Err(GraphicsError::NonCanonicalEncoding);
        }
        Ok(scene)
    }
}

fn validate_rect(rect: LayoutRect) -> Result<(), GraphicsError> {
    if rect.width == 0
        || rect.height == 0
        || rect.width > MAX_LAYOUT_EXTENT
        || rect.height > MAX_LAYOUT_EXTENT
    {
        return Err(GraphicsError::InvalidGeometry);
    }
    let right = i32::from(rect.x) + i32::from(rect.width);
    let bottom = i32::from(rect.y) + i32::from(rect.height);
    if right > i32::from(i16::MAX) || bottom > i32::from(i16::MAX) {
        return Err(GraphicsError::InvalidGeometry);
    }
    Ok(())
}

fn intersection(a: LayoutRect, b: LayoutRect) -> Option<LayoutRect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (i32::from(a.x) + i32::from(a.width)).min(i32::from(b.x) + i32::from(b.width));
    let bottom = (i32::from(a.y) + i32::from(a.height)).min(i32::from(b.y) + i32::from(b.height));
    if right <= i32::from(x) || bottom <= i32::from(y) {
        return None;
    }
    Some(LayoutRect {
        x,
        y,
        width: (right - i32::from(x)) as u16,
        height: (bottom - i32::from(y)) as u16,
    })
}

fn write_rect(output: &mut [u8], rect: LayoutRect) {
    output[0..2].copy_from_slice(&rect.x.to_le_bytes());
    output[2..4].copy_from_slice(&rect.y.to_le_bytes());
    output[4..6].copy_from_slice(&rect.width.to_le_bytes());
    output[6..8].copy_from_slice(&rect.height.to_le_bytes());
}
fn read_rect(input: &[u8]) -> LayoutRect {
    LayoutRect {
        x: i16::from_le_bytes([input[0], input[1]]),
        y: i16::from_le_bytes([input[2], input[3]]),
        width: u16::from_le_bytes([input[4], input[5]]),
        height: u16::from_le_bytes([input[6], input[7]]),
    }
}
fn decode_kind(value: u8) -> Result<GraphicsCommandKind, GraphicsError> {
    match value {
        1 => Ok(GraphicsCommandKind::Rect),
        2 => Ok(GraphicsCommandKind::Text),
        3 => Ok(GraphicsCommandKind::Icon),
        _ => Err(GraphicsError::MalformedEncoding),
    }
}
fn decode_paint(value: u8) -> Result<GraphicsPaintRole, GraphicsError> {
    match value {
        1 => Ok(GraphicsPaintRole::Background),
        2 => Ok(GraphicsPaintRole::Foreground),
        3 => Ok(GraphicsPaintRole::Accent),
        4 => Ok(GraphicsPaintRole::Status),
        _ => Err(GraphicsError::MalformedEncoding),
    }
}
fn decode_style(value: u8) -> Result<GraphicsShapeStyle, GraphicsError> {
    match value {
        1 => Ok(GraphicsShapeStyle::Fill),
        2 => Ok(GraphicsShapeStyle::Stroke),
        _ => Err(GraphicsError::MalformedEncoding),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(x: i16, width: u16) -> LayoutRect {
        LayoutRect {
            x,
            y: 0,
            width,
            height: 10,
        }
    }

    #[test]
    fn round_trip_and_clip_classes_are_raster_independent() {
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::rect(
                    rect(0, 10),
                    rect(0, 10),
                    GraphicsPaintRole::Background,
                    GraphicsShapeStyle::Fill,
                )
                .unwrap(),
            )
            .unwrap();
        scene
            .push(
                GraphicsCommand::text(
                    rect(5, 10),
                    rect(0, 10),
                    GraphicsPaintRole::Foreground,
                    "ready",
                )
                .unwrap(),
            )
            .unwrap();
        scene
            .push(
                GraphicsCommand::icon(
                    rect(20, 5),
                    rect(0, 10),
                    GraphicsPaintRole::Accent,
                    PresentationIconKey::Presentation,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            scene
                .commands()
                .iter()
                .map(GraphicsCommand::clip_class)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                GraphicsClipClass::FullyVisible,
                GraphicsClipClass::PartiallyClipped,
                GraphicsClipClass::FullyClipped
            ]
        );
        let encoded = scene.encode();
        assert_eq!(
            GraphicsScene::decode(&encoded[..scene.encoded_len()]),
            Ok(scene)
        );
    }

    #[test]
    fn malformed_overflow_and_unknown_icon_refuse() {
        assert_eq!(
            GraphicsCommand::text(rect(0, 0), rect(0, 1), GraphicsPaintRole::Foreground, "x"),
            Err(GraphicsError::InvalidGeometry)
        );
        assert_eq!(
            GraphicsCommand::new(
                GraphicsCommandKind::Icon,
                rect(0, 1),
                rect(0, 1),
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Fill,
                b"invented"
            ),
            Err(GraphicsError::UnknownIcon)
        );
        let mut encoded = [0; 3];
        encoded[0] = VERSION;
        assert_eq!(
            GraphicsScene::decode(&encoded),
            Err(GraphicsError::NonCanonicalEncoding)
        );
    }
}
