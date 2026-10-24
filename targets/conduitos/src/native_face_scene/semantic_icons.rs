//! Finite Mask decoration from explicit semantic roles, never names or action IDs.
use super::*;
use conduit_face::{FaceIconKey, FaceRole};

pub(super) fn for_role(role: &FaceRole) -> Option<FaceIconKey> {
    Some(match role {
        FaceRole::Body => FaceIconKey::Body,
        FaceRole::Plot => FaceIconKey::Face,
        FaceRole::Document => FaceIconKey::Type,
        FaceRole::Host => FaceIconKey::Host,
        FaceRole::Gear => FaceIconKey::GenericGear,
        FaceRole::Port => FaceIconKey::Port,
        FaceRole::Line | FaceRole::Cord => FaceIconKey::Line,
        FaceRole::Plan => FaceIconKey::Plan,
        FaceRole::Play => FaceIconKey::Play,
        FaceRole::Status => FaceIconKey::Status,
        FaceRole::Diagnostic => FaceIconKey::Warning,
        _ => return None,
    })
}

/// Explicit owner-offered intents choose decoration, never captions or opaque IDs.
pub(super) fn for_intent(intent: &str) -> Option<FaceIconKey> {
    Some(match intent {
        "conduit.intent/birth@1" | "conduit.intent/creche.birth@1" | "conduit.intent/wake@1" => {
            FaceIconKey::Wake
        }
        "conduit.intent/creche.name@1" | "patchbay/edit-subject@1" => FaceIconKey::Type,
        "conduit.intent/creche.naming@1" => FaceIconKey::GenericGear,
        "conduit.intent/creche.suggest@1" | "todo/reopen@1" => FaceIconKey::Repeat2,
        "conduit.intent/open-inspection@1" | "conduit.intent/inspect@1" => FaceIconKey::Status,
        "conduit.intent/fulfill@1" | "todo/complete@1" => FaceIconKey::Confirm,
        "conduit.intent/open-body-overview@1" => FaceIconKey::Body,
        "conduit.intent/open-resident-plot@1" | "patchbay/select-plot@1" => FaceIconKey::Face,
        _ => return None,
    })
}

pub(super) fn inset(icon: Option<FaceIconKey>) -> u16 {
    if icon.is_some() { 24 } else { 0 }
}

fn bounds(row: &layout::Row) -> LayoutRect {
    let size = 16.min(row.bounds.height - row.padding * 2);
    LayoutRect {
        x: row.bounds.x + row.padding as i16,
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
        for intent in [
            "Birth Body",
            "creche.birth",
            "custom/conduit.intent/creche.birth@1",
            "conduit.intent/creche.birth@2",
        ] {
            assert_eq!(for_intent(intent), None);
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
