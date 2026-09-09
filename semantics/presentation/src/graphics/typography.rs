//! Text purpose without font-family, size, or pixel facts.
use super::GraphicsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphicsTextRole {
    Body = 1,
    Title = 2,
    Heading = 3,
    Label = 4,
    Code = 5,
    Status = 6,
    Warning = 7,
    Action = 8,
    Muted = 9,
}

impl GraphicsTextRole {
    pub(super) fn decode(value: u8) -> Result<Self, GraphicsError> {
        match value {
            1 => Ok(Self::Body),
            2 => Ok(Self::Title),
            3 => Ok(Self::Heading),
            4 => Ok(Self::Label),
            5 => Ok(Self::Code),
            6 => Ok(Self::Status),
            7 => Ok(Self::Warning),
            8 => Ok(Self::Action),
            9 => Ok(Self::Muted),
            _ => Err(GraphicsError::MalformedEncoding),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphicsCommand, GraphicsPaintRole, GraphicsScene, LayoutRect};

    #[test]
    fn text_roles_round_trip_and_legacy_body_remains_decodable() {
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: 200,
            height: 80,
        };
        for role in [
            GraphicsTextRole::Body,
            GraphicsTextRole::Title,
            GraphicsTextRole::Heading,
            GraphicsTextRole::Label,
            GraphicsTextRole::Code,
            GraphicsTextRole::Status,
            GraphicsTextRole::Warning,
            GraphicsTextRole::Action,
            GraphicsTextRole::Muted,
        ] {
            let mut scene = GraphicsScene::empty();
            scene
                .push(
                    GraphicsCommand::text(
                        bounds,
                        bounds,
                        GraphicsPaintRole::Foreground,
                        "café / id:42",
                    )
                    .unwrap()
                    .with_text_role(role)
                    .unwrap(),
                )
                .unwrap();
            let mut bytes = scene.encode();
            let len = scene.encoded_len();
            assert_eq!(GraphicsScene::decode(&bytes[..len]), Ok(scene));
            bytes[len - 1] = 255;
            assert_eq!(
                GraphicsScene::decode(&bytes[..len]),
                Err(GraphicsError::MalformedEncoding)
            );
            bytes[0] = 1;
            let legacy = GraphicsScene::decode(&bytes[..len - 1]).unwrap();
            assert_eq!(legacy.commands()[0].text_role(), GraphicsTextRole::Body);
            assert_eq!(legacy.commands()[0].payload(), "café / id:42");
        }
    }
}
