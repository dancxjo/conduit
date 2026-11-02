use super::*;
use crate::display::{DisplayError, DisplayFormat};
use conduit_face::ManifestationLifecycle;
use conduit_human::{KeyModifiers, KeyTransition};
struct BufferedDisplay {
    writes: u64,
    commits: u64,
    fail_commit: bool,
}
impl PixelTarget for BufferedDisplay {
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
        self.writes += 1;
        Ok(())
    }
    fn commit_frame(&mut self) -> Result<(), DisplayError> {
        self.commits += 1;
        if self.fail_commit {
            Err(DisplayError::Lost)
        } else {
            Ok(())
        }
    }
}
#[test]
fn cpu_writes_without_successful_present_never_acknowledge_available_show() {
    let (mut mask, draft) = super::super::tests::fixture();
    let face = draft.face(&mask.birth_basis("commit/initial")).unwrap();
    let mut display = BufferedDisplay {
        writes: 0,
        commits: 0,
        fail_commit: true,
    };
    // Also verify &mut PixelTarget forwarding preserves its commit boundary.
    assert!(mask.present(face.clone(), 1, 1, &mut &mut display).is_err());
    assert!(display.writes > 0 && display.commits == 1);
    assert!(mask.show().is_none() && mask.render_receipt().is_none());
    assert_eq!(mask.route_keyboard(), Err(NativeFaceMaskError::Face));
    display.fail_commit = false;
    assert!(
        mask.show().is_none(),
        "restored output availability alone cannot acknowledge"
    );
    let writes = display.writes;
    mask.present(face, 2, 2, &mut display).unwrap();
    assert!(display.writes > writes);
    assert_eq!(
        mask.show().unwrap().show.lifecycle,
        ManifestationLifecycle::Available
    );
}
#[test]
fn repaint_commit_failure_revokes_the_old_show_until_fresh_render() {
    let (mut mask, draft) = super::super::tests::fixture();
    let face = draft.face(&mask.birth_basis("commit/repaint")).unwrap();
    let mut display = BufferedDisplay {
        writes: 0,
        commits: 0,
        fail_commit: false,
    };
    mask.present(face.clone(), 1, 1, &mut display).unwrap();
    let old = mask.show().unwrap().clone();
    let writes = display.writes;
    display.fail_commit = true;
    let tab = KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(mask.key(tab, 1, &mut display).is_err());
    assert!(display.writes > writes);
    assert!(mask.show().is_none() && mask.render_receipt().is_none());
    assert_eq!(
        mask.scene().unwrap().face(),
        &face,
        "semantic Face survives output loss without Show authority"
    );
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "creche.suggest")
        .unwrap();
    let stale = FaceInteraction::new(
        &face,
        &old,
        &action.identity,
        &action.target,
        alloc::vec![],
        2,
    )
    .unwrap();
    assert_eq!(
        mask.submit(stale),
        Err(NativeFaceMaskError::InputUnavailable)
    );
    display.fail_commit = false;
    assert_eq!(mask.route_keyboard(), Err(NativeFaceMaskError::Face));
    mask.present(face, 2, 2, &mut display).unwrap();
    assert_ne!(mask.show().unwrap().show_id, old.show_id);
}
