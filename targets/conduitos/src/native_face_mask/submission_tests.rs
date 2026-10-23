//! Actual typed submission and notice scanout on a CPU target; not QEMU proof.
use super::*;
use crate::display::{DisplayError, DisplayFormat};
use conduit_birth_plot::BirthDraft;
use conduit_face::FaceInteractionRefusal;

#[derive(Default)]
struct Target {
    writes: u32,
    fail: bool,
}
impl PixelTarget for Target {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        if self.fail {
            return Err(DisplayError::Lost);
        }
        self.writes += 1;
        Ok(())
    }
}
fn suggest(mask: &NativeFaceMask, sequence: u64) -> FaceInteraction {
    let scene = mask.scene().unwrap();
    let action = scene
        .face()
        .actions
        .iter()
        .position(|action| action.identity == "creche.suggest")
        .unwrap();
    scene
        .interaction(
            &scene.face().identity,
            scene.face().revision,
            crate::native_face_scene::FaceControl {
                action,
                argument: None,
            },
            mask.show().unwrap(),
            alloc::vec![],
            sequence,
        )
        .unwrap()
}
fn replacement() -> (NativeFaceMask, BirthDraft, Target, FaceInteraction) {
    let (mut old, draft) = super::tests::fixture();
    let face = draft.face(&old.birth_basis("arrival/submission")).unwrap();
    let mut target = Target::default();
    old.present(face.clone(), 1, 1, &mut target).unwrap();
    let held = suggest(&old, 1);
    let first_show = old.show().unwrap().clone();
    drop(old);
    let (mut fresh, _) = super::tests::fixture();
    assert_eq!(
        draft
            .face(&fresh.birth_basis("arrival/submission"))
            .unwrap(),
        face
    );
    fresh.present(face.clone(), 2, 2, &mut target).unwrap();
    let second_show = fresh.show().unwrap();
    assert_ne!(first_show.show_id, second_show.show_id);
    assert_eq!(first_show.face_id, second_show.face_id);
    assert_eq!(first_show.face_revision, second_show.face_revision);
    assert_eq!(
        held.validate_against(&face, second_show),
        Err(FaceInteractionRefusal::StaleShow)
    );
    (fresh, draft, target, held)
}
#[test]
fn stale_show_notice_preserves_fresh_session_then_real_source_action_succeeds() {
    let (mut mask, mut draft, mut target, held) = replacement();
    let face = mask.scene().unwrap().face().clone();
    let show = mask.show().unwrap().clone();
    let publication = mask.publication().unwrap().clone();
    let revision = draft.revision();
    let writes = target.writes;
    assert_eq!(
        mask.submit_with_display(held, &mut target).err(),
        Some(NativeFaceMaskError::Mask(NativeMaskPlayError::Interaction(
            FaceInteractionRefusal::StaleShow
        )))
    );
    assert_eq!(mask.show(), Some(&show));
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert_eq!(mask.publication(), Some(&publication));
    assert_eq!(draft.revision(), revision);
    assert!(target.writes > writes);
    assert!(
        mask.scene()
            .unwrap()
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Stale: refresh")
    );
    assert_eq!(mask.route_keyboard(), Ok(true));
    // No invented SceneError/notice-custody record is substituted for the typed
    // Interaction(StaleShow) returned by the real pending Mask Play.
    assert!(mask.acknowledged_input_refusal().is_none());
    let fresh = suggest(&mask, 2);
    let basis = mask.birth_basis("arrival/submission");
    let correlation = mask
        .submit_with_display(fresh.clone(), &mut target)
        .unwrap();
    assert_eq!(correlation.interaction, fresh);
    assert_eq!(correlation.show_id, show.show_id);
    assert!(mask.show().is_none());
    draft
        .apply_face_interaction(&basis, &show, &correlation.interaction)
        .unwrap();
    assert!(draft.revision() > revision);
}
#[test]
fn stale_notice_scanout_loss_retires_session_and_returns_display_failure() {
    let (mut mask, draft, mut target, held) = replacement();
    let revision = draft.revision();
    let face = mask.scene().unwrap().face().clone();
    target.fail = true;
    assert!(matches!(
        mask.submit_with_display(held, &mut target),
        Err(NativeFaceMaskError::Compositor(
            crate::native_compositor::NativeCompositorError::Display(DisplayError::Lost)
        ))
    ));
    assert!(mask.show().is_none());
    assert!(mask.route_keyboard().is_err());
    assert!(mask.acknowledged_input_refusal().is_none());
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert_eq!(draft.revision(), revision);
}
