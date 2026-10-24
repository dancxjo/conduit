//! Port decoration uses only measured spare ink space and residual commands.
use super::*;
use crate::display::typography::TextRole;
use crate::native_face_scene::{chrome, semantic_icons};
use conduit_face::MAX_GRAPHICS_COMMANDS;
const RESERVE: u16 = 22;

pub(super) fn admit(
    face: &Face,
    gears: &[&FaceSubject],
    height: u16,
    width: u16,
    used: usize,
) -> Result<bool, FaceSceneError> {
    let Some(available) = width.checked_sub(24 + RESERVE).filter(|width| *width >= 32) else {
        return Ok(false);
    };
    let count: usize = gears
        .iter()
        .map(|gear| {
            let count = ports(face, gear).count();
            visible_ports(height, count).min(count)
        })
        .sum();
    if count == 0 || used + count + chrome::COMMANDS > MAX_GRAPHICS_COMMANDS {
        return Ok(false);
    }
    // Every visible Port receives the same treatment, or every one stays text-only.
    // Compare exact prepared payloads: decoration never asks for a shorter label.
    for gear in gears {
        let count = ports(face, gear).count();
        for port in ports(face, gear).take(visible_ports(height, count)) {
            let label = preview(
                &super::super::ports::label(face, port),
                TextRole::Label,
                width - 24,
            )?;
            if preview(&label, TextRole::Label, available)? != label {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn append(
    scene: &mut GraphicsScene,
    row: LayoutRect,
    screen: LayoutRect,
    paint: GraphicsPaintRole,
) -> Result<(), FaceSceneError> {
    let icon = semantic_icons::for_role(&FaceRole::Port).ok_or(FaceSceneError::InvalidFace)?;
    push(
        scene,
        GraphicsCommand::icon(
            LayoutRect {
                x: row.x + row.width as i16 - 16,
                y: row.y + 4,
                width: 16,
                height: 16,
            },
            screen,
            paint,
            icon,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{FaceIconKey, GraphicsCommandKind};
    fn fixture() -> Face {
        serde_json::from_str(include_str!(
            "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
        ))
        .unwrap()
    }
    #[test]
    fn port_symbols_keep_original_labels_links_and_hit_regions() {
        let face = fixture();
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        };
        let mut scene = GraphicsScene::empty();
        let hits = super::super::append(&mut scene, &face, screen, 0).unwrap();
        let symbols: Vec<_> = scene
            .commands()
            .iter()
            .filter(|command| {
                command.kind == GraphicsCommandKind::Icon
                    && command.payload() == FaceIconKey::Port.as_str()
            })
            .collect();
        assert_eq!(symbols.len(), 4);
        for symbol in symbols {
            assert!(
                hits.ports
                    .iter()
                    .flatten()
                    .any(|(row, _)| symbol.bounds.x >= row.x
                        && symbol.bounds.y >= row.y
                        && symbol.bounds.x + symbol.bounds.width as i16
                            <= row.x + row.width as i16
                        && symbol.bounds.y + symbol.bounds.height as i16
                            <= row.y + row.height as i16)
            );
        }
        assert_eq!(hits.ports.iter().flatten().count(), 4);
        assert_eq!(
            scene
                .commands()
                .iter()
                .filter(|command| command.kind == GraphicsCommandKind::OrthogonalPath)
                .count(),
            2
        );
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| command.payload().starts_with("IN value/text"))
        );
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| command.payload().starts_with("OUT value/text"))
        );
        assert!(scene.commands().len() + chrome::COMMANDS <= MAX_GRAPHICS_COMMANDS);
    }
    #[test]
    fn crowded_labels_refuse_decoration_instead_of_shortening_text() {
        let face = fixture();
        let gears: Vec<_> = face
            .subjects
            .iter()
            .filter(|s| s.role == FaceRole::Gear)
            .collect();
        assert!(!admit(&face, &gears, 180, 100, 0).unwrap());
        assert!(
            !admit(
                &face,
                &gears,
                180,
                400,
                MAX_GRAPHICS_COMMANDS - chrome::COMMANDS
            )
            .unwrap()
        );
    }
}
