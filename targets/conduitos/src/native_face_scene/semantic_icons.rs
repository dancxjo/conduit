//! Finite Mask decoration from explicit semantic roles, never names or action IDs.
use super::*;
use conduit_face::FaceIconKey;

#[path = "semantic_icons/generated.rs"]
mod mapping;
pub(super) use mapping::{for_intent, for_role};

/// Match the admitted user text scale; no new asset or frame command is needed.
pub(super) fn size(scale: u16) -> u16 {
    (16 * u32::from(scale)).div_ceil(100) as u16
}

pub(super) fn inset(icon: Option<FaceIconKey>, size: u16) -> u16 {
    if icon.is_some() {
        size + size.div_ceil(2)
    } else {
        0
    }
}

fn bounds(row: &layout::Row) -> LayoutRect {
    let size = row
        .icon_size
        .min(row.bounds.height - row.insets.block() * 2);
    LayoutRect {
        x: row.bounds.x + row.insets.inline() as i16 + row.insets.leading() as i16,
        y: row.bounds.y + ((row.bounds.height - size) / 2) as i16,
        width: size,
        height: size,
    }
}

pub(super) fn append(
    scene: &mut GraphicsScene,
    row: &layout::Row,
    screen: LayoutRect,
) -> Result<(), FaceSceneError> {
    if let Some(icon) = row.icon {
        push(
            scene,
            GraphicsCommand::icon(bounds(row), screen, row.text_paint(), icon),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::FaceRole;

    #[test]
    fn roles_select_only_truthful_existing_icons_without_name_inference() {
        for (role, icon) in [
            (FaceRole::Body, FaceIconKey::Body),
            (FaceRole::Plot, FaceIconKey::Face),
            (FaceRole::Document, FaceIconKey::Type),
            (FaceRole::Host, FaceIconKey::Host),
            (FaceRole::Diagnostic, FaceIconKey::Warning),
            (FaceRole::Status, FaceIconKey::Status),
        ] {
            assert_eq!(for_role(&role), Some(icon));
        }
        for role in [
            FaceRole::Action,
            FaceRole::Info,
            FaceRole::Item,
            FaceRole::Semantic(conduit_core::kind_id("custom/warning")),
        ] {
            assert_eq!(for_role(&role), None);
        }
    }

    #[test]
    fn action_decoration_requires_an_exact_offered_intent() {
        assert_eq!(
            for_intent("conduit.intent/creche.birth@1"),
            Some(FaceIconKey::Wake)
        );
        assert_eq!(
            for_intent("conduit.intent/creche.name@1"),
            Some(FaceIconKey::Type)
        );
        assert_eq!(for_intent("todo/add@1"), Some(FaceIconKey::Add));
        assert_eq!(for_intent("todo/remove@1"), Some(FaceIconKey::Remove));
        for intent in [
            "todo/add@2",
            "custom/todo/remove@1",
            "Birth Body",
            "creche.birth",
            "custom/conduit.intent/creche.birth@1",
            "conduit.intent/creche.birth@2",
        ] {
            assert_eq!(for_intent(intent), None);
        }
    }

    #[test]
    fn details_reuses_explicit_mappings_without_changing_clauses_or_read_only_binding() {
        let mut face = super::super::tests::face(7);
        face.subjects[0].role = FaceRole::Host;
        face.actions[0].intent = "conduit.intent/creche.name@1".into();
        // A familiar caption cannot select an icon for an unknown intent.
        face.actions[1].name = "Search Plots".into();
        let face = Face::new_with_semantics(
            face.revision,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            face.actions,
            face.disclosures,
        )
        .unwrap();
        let clauses = conduit_face::plan_face_utterances(&face).unwrap();
        for admitted in [true, false] {
            let (_, details) = document::prepare(&face, admitted).unwrap();
            assert_eq!(
                details
                    .iter()
                    .map(|row| row.text.as_str())
                    .collect::<Vec<_>>(),
                clauses
                    .clauses
                    .iter()
                    .map(|clause| clause.text.as_str())
                    .collect::<Vec<_>>()
            );
            assert!(
                details
                    .iter()
                    .any(|row| row.icon == Some(FaceIconKey::Host))
            );
            assert_eq!(
                details
                    .iter()
                    .filter(|row| row.icon == Some(FaceIconKey::Type))
                    .count(),
                1
            );
            if admitted {
                let action = details
                    .iter()
                    .find(|row| {
                        row.control
                            == Some(FaceControl {
                                action: 0,
                                argument: None,
                            })
                    })
                    .unwrap();
                assert_eq!(action.icon, Some(FaceIconKey::Type));
                assert!(
                    details
                        .iter()
                        .filter(|row| row.control.is_some_and(
                            |control| control.argument.is_some() || control.action == 1
                        ))
                        .all(|row| row.icon.is_none())
                );
            } else {
                assert!(details.iter().all(|row| row.control.is_none()));
            }
        }
    }

    #[test]
    fn icons_are_measured_before_wrap_and_fit_bounded_pages_without_semantic_changes() {
        let mut face = super::super::tests::face(7);
        face.subjects[0].role = FaceRole::Host;
        let face = Face::new_with_semantics(
            face.revision,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            face.actions,
            face.disclosures,
        )
        .unwrap();
        for scale in [100, 125, 200] {
            let (items, _) = document::prepare(&face, true).unwrap();
            let rows = layout::admit_with_expression(
                items,
                LayoutRect {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 240,
                },
                SPACE_XL,
                1,
                scale,
                SPACE_SM,
            )
            .unwrap();
            assert!(rows.iter().any(|row| row.icon == Some(FaceIconKey::Host)));
            for row in rows.iter().filter(|row| row.icon.is_some()) {
                let icon = bounds(row);
                assert_eq!(
                    icon.width,
                    16 * scale / 100,
                    "icon must follow admitted text scale"
                );
                assert!(
                    i32::from(icon.x) + i32::from(icon.width) <= i32::from(row.text_bounds().x)
                );
                assert!(icon.y >= row.bounds.y);
                assert!(
                    i32::from(icon.y) + i32::from(icon.height)
                        <= i32::from(row.bounds.y) + i32::from(row.bounds.height)
                );
            }
        }
        let mut scene = NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
        for page in 0..scene.pages() {
            scene.page = page;
            scene.frame().unwrap();
            assert_eq!(scene.face(), &face);
        }
    }
}
