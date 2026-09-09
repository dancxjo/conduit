//! Independent native-software realization of canonical graphics obligations.
//!
//! The browser presenter may bypass this leaf layer by joining semantic
//! presentation directly. A ConduitOS framebuffer presenter can consume the
//! same integer rectangles, clip classes, paint roles, ordering, resolved text,
//! and canonical icon keys before performing its own admitted raster writes;
//! framebuffer addresses remain below this contract.

use conduit_presentation::{
    GraphicsClipClass, GraphicsCommandKind, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle,
    LayoutRect, PresentationIconKey, MAX_GRAPHICS_COMMANDS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeGraphicsObligation<'a> {
    pub order: u8,
    pub kind: GraphicsCommandKind,
    pub bounds: LayoutRect,
    pub clip: GraphicsClipClass,
    pub paint: GraphicsPaintRole,
    pub style: GraphicsShapeStyle,
    pub text_role: conduit_presentation::GraphicsTextRole,
    pub resolved_content: &'a str,
    pub path: Option<conduit_presentation::GraphicsPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeGraphicsError {
    UnknownIcon,
    CapacityExceeded,
}

pub struct NativeGraphicsPresenter;

impl NativeGraphicsPresenter {
    pub fn normalize<'a>(
        scene: &'a GraphicsScene,
    ) -> Result<[Option<NativeGraphicsObligation<'a>>; MAX_GRAPHICS_COMMANDS], NativeGraphicsError>
    {
        let mut output = [None; MAX_GRAPHICS_COMMANDS];
        for (index, command) in scene.commands().iter().enumerate() {
            if index == MAX_GRAPHICS_COMMANDS {
                return Err(NativeGraphicsError::CapacityExceeded);
            }
            let content = command.payload();
            if command.kind == GraphicsCommandKind::Icon
                && PresentationIconKey::from_token(content).is_none()
            {
                return Err(NativeGraphicsError::UnknownIcon);
            }
            output[index] = Some(NativeGraphicsObligation {
                order: index as u8,
                kind: command.kind,
                bounds: command.bounds,
                clip: command.clip_class(),
                paint: command.paint,
                style: command.style,
                text_role: command.text_role,
                resolved_content: content,
                path: command.path_geometry(),
            });
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::{GraphicsCommand, GraphicsScene};

    #[test]
    fn path_geometry_survives_normalization_without_becoming_text() {
        use conduit_presentation::{GraphicsPath, GraphicsPoint};
        let path = GraphicsPath::new(&[
            GraphicsPoint { x: 1, y: 2 },
            GraphicsPoint { x: 8, y: 2 },
            GraphicsPoint { x: 8, y: 9 },
        ])
        .unwrap();
        let clip = LayoutRect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(GraphicsCommand::path(path, clip, GraphicsPaintRole::Status).unwrap())
            .unwrap();
        let normalized = NativeGraphicsPresenter::normalize(&scene).unwrap()[0].unwrap();
        assert_eq!(normalized.path, Some(path));
        assert_eq!(normalized.resolved_content, "");
        assert_eq!(normalized.kind, GraphicsCommandKind::OrthogonalPath);
    }

    #[test]
    fn native_normalizer_preserves_semantics_without_pixel_parity() {
        let bounds = LayoutRect {
            x: 8,
            y: 8,
            width: 24,
            height: 12,
        };
        let clip = LayoutRect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::text(bounds, clip, GraphicsPaintRole::Foreground, "ready")
                    .unwrap(),
            )
            .unwrap();
        scene
            .push(
                GraphicsCommand::icon(
                    bounds,
                    clip,
                    GraphicsPaintRole::Accent,
                    PresentationIconKey::Presentation,
                )
                .unwrap(),
            )
            .unwrap();
        let normalized = NativeGraphicsPresenter::normalize(&scene).unwrap();
        assert_eq!(normalized[0].unwrap().resolved_content, "ready");
        assert_eq!(
            normalized[0].unwrap().clip,
            GraphicsClipClass::PartiallyClipped
        );
        assert_eq!(normalized[1].unwrap().resolved_content, "presentation");
        assert_eq!(normalized[1].unwrap().order, 1);
    }
}
