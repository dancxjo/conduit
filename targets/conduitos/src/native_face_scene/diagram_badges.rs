//! Optional prepared Lucide decoration after mandatory graph geometry is admitted.
use super::*;
use crate::native_face_scene::{chrome, semantic_icons};
use conduit_face::MAX_GRAPHICS_COMMANDS;

pub(super) const INSET: u16 = 24;

/// All visible cards use the same treatment. Dense/small pages retain the
/// complete labeled graph instead of trading meaningful geometry for ornament.
pub(super) fn admit(
    cards: usize,
    links: usize,
    gears: &[&FaceSubject],
    width: u16,
    height: u16,
) -> Result<bool, FaceSceneError> {
    if width < 112
        || height < 48
        || gears.is_empty()
        || cards + links + gears.len() + chrome::COMMANDS > MAX_GRAPHICS_COMMANDS
    {
        return Ok(false);
    }
    // As with Port decoration, retain the exact text-only prepared payload.
    // If any heading would shorten, the entire page keeps its labeled cards.
    for (index, gear) in gears.iter().enumerate() {
        let (title, _) = gear_labels(&gear.name);
        let label = preview(
            &alloc::format!("{} · {}", index + 1, readable(title)),
            crate::display::typography::TextRole::Heading,
            width - 20,
        )?;
        if preview(
            &label,
            crate::display::typography::TextRole::Heading,
            width - 20 - INSET,
        )? != label
        {
            return Ok(false);
        }
    }
    Ok(true)
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
        check_graph_symbols(640, 0);
        check_graph_symbols(1280, 3);
    }

    fn check_graph_symbols(width: u16, gear_symbols: usize) {
        let face = fixture();
        let mut scene = NativeFaceScene::prepare(face.clone(), width, 720).unwrap();
        scene.show_diagram(true);
        let frame = scene.frame().unwrap();
        let commands = frame.scene.commands();
        let icons: Vec<_> = commands
            .iter()
            .filter(|c| c.kind == GraphicsCommandKind::Icon)
            .collect();
        assert_eq!(
            icons
                .iter()
                .filter(|c| c.payload() == FaceIconKey::GenericGear.as_str())
                .count(),
            gear_symbols
        );
        let baseline_width = if width == 640 { 260 } else { 580 };
        let first_heading = crate::display::typography::single_line_preview(
            "1 · Literal 6f8beb2a9ae7 0",
            crate::display::typography::TextRole::Heading,
            baseline_width,
        )
        .unwrap();
        assert!(commands.iter().any(|command| {
            command.text_role() == GraphicsTextRole::Heading && command.payload() == first_heading
        }));
        assert!(icons.iter().all(|c| {
            [
                FaceIconKey::GenericGear.as_str(),
                FaceIconKey::Port.as_str(),
            ]
            .contains(&c.payload())
        }));
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
