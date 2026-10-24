//! Optional prepared Lucide decoration after mandatory graph geometry is admitted.
use super::*;
use crate::native_face_scene::{chrome, semantic_icons};
use conduit_face::MAX_GRAPHICS_COMMANDS;

pub(super) const INSET: u16 = 24;

/// All visible cards use the same treatment. Dense/small pages retain the
/// complete labeled graph instead of trading meaningful geometry for ornament.
pub(super) fn admit(cards: usize, links: usize, count: usize, width: u16, height: u16) -> bool {
    width >= 112
        && height >= 48
        && count > 0
        && cards + links + count + chrome::COMMANDS <= MAX_GRAPHICS_COMMANDS
}

pub(super) fn append(
    scene: &mut GraphicsScene,
    card: LayoutRect,
    screen: LayoutRect,
) -> Result<(), FaceSceneError> {
    let icon = semantic_icons::for_role(&FaceRole::Gear).ok_or(FaceSceneError::InvalidFace)?;
    push(
        scene,
        GraphicsCommand::icon(
            LayoutRect {
                x: card.x + 10,
                y: card.y + 14,
                width: 16,
                height: 16,
            },
            screen,
            GraphicsPaintRole::Accent,
            icon,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_face_scene::NativeFaceScene;
    use conduit_face::{FaceIconKey, FaceRelationship, GraphicsCommandKind};

    fn fixture() -> Face {
        serde_json::from_str(include_str!(
            "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
        ))
        .unwrap()
    }

    #[test]
    fn graph_cards_use_shared_gear_symbols_without_changing_links_or_subjects() {
        let face = fixture();
        let mut scene = NativeFaceScene::prepare(face.clone(), 640, 720).unwrap();
        scene.show_diagram(true);
        let frame = scene.frame().unwrap();
        let commands = frame.scene.commands();
        let icons: Vec<_> = commands
            .iter()
            .filter(|c| c.kind == GraphicsCommandKind::Icon)
            .collect();
        assert_eq!(icons.len(), 3);
        assert!(
            icons
                .iter()
                .all(|c| c.payload() == FaceIconKey::GenericGear.as_str())
        );
        assert_eq!(
            commands
                .iter()
                .filter(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
                .count(),
            2
        );
        assert!(
            commands
                .iter()
                .any(|c| c.payload().starts_with("IN value/text"))
        );
        assert!(
            commands
                .iter()
                .any(|c| c.payload().starts_with("OUT value/text"))
        );
        assert_eq!(scene.face(), &face);
    }

    #[test]
    fn saturated_graph_keeps_every_card_and_port_and_falls_back_to_labeled_cards() {
        let original = fixture();
        let mut subjects = Vec::new();
        let mut relationships = Vec::new();
        for gear in 0..6 {
            let id = alloc::format!("gear/{gear}");
            subjects.push(FaceSubject {
                identity: id.clone(),
                role: FaceRole::Gear,
                name: alloc::format!("bounded/node-{gear} Gear"),
            });
            for port in 0..4 {
                let target = alloc::format!("{id}/port/{port}");
                subjects.push(FaceSubject {
                    identity: target.clone(),
                    role: FaceRole::Port,
                    name: alloc::format!("port {port}"),
                });
                relationships.push(FaceRelationship {
                    source: id.clone(),
                    target,
                    kind: FaceRelationshipKind::Contains,
                });
            }
        }
        let face = Face::new_with_semantics(
            original.revision,
            original.basis,
            subjects,
            relationships,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: 1280,
            height: 900,
        };
        let mut graphics = GraphicsScene::empty();
        let hits = super::super::append(&mut graphics, &face, screen, 0).unwrap();
        assert_eq!(hits.gears.iter().flatten().count(), 6);
        assert_eq!(hits.ports.iter().flatten().count(), 24);
        assert_eq!(
            graphics.commands().len() + chrome::COMMANDS,
            MAX_GRAPHICS_COMMANDS
        );
        assert!(
            graphics
                .commands()
                .iter()
                .all(|c| c.kind != GraphicsCommandKind::Icon)
        );
        for gear in 0..6 {
            let label = alloc::format!("{} · Node {gear}", gear + 1);
            assert!(graphics.commands().iter().any(|c| c.payload() == label));
        }
    }
}
