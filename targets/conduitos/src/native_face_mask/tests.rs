//! Product-path Face-to-Mask scanout and interaction proof.

use super::*;
use alloc::vec;
use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_human::{KeyModifiers, KeyTransition};
use conduit_presentation::ManifestationLifecycle;

struct CountingDisplay {
    writes: u32,
    fail: bool,
}

impl PixelTarget for CountingDisplay {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }

    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), crate::display::DisplayError> {
        if self.fail {
            return Err(crate::display::DisplayError::Lost);
        }
        self.writes += 1;
        Ok(())
    }
}

fn fixture() -> (NativeFaceMask, BirthDraft) {
    let provider = crate::product_bases::fixture_surface_provider();
    let service = NativeFaceMask::prepare(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        "surface/native-face",
        &provider,
    )
    .unwrap();
    let choices = crate::native_workset::profile()
        .installed()
        .iter()
        .map(|plot| BirthPlotChoice {
            title: plot.title().into(),
            search_text: plot.title().into(),
            plot: crate::native_workset::resident(*plot).unwrap(),
            refusal: None,
            selected: true,
        })
        .collect();
    let draft = BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    (service, draft)
}

#[test]
fn birth_face_crosses_producer_mask_scanout_and_exact_interaction_fore() {
    let (mut service, mut draft) = fixture();
    let basis = service.birth_basis("arrival/one");
    let face = draft.face(&basis).unwrap();
    let mut display = CountingDisplay {
        writes: 0,
        fail: false,
    };
    let composition = service.present(face.clone(), 1, 1, &mut display).unwrap();
    let publication = service.publication().unwrap();
    assert_eq!(publication.presentation_id, face.identity.as_str());
    assert!(publication.kernel_signs > 0);
    assert!(display.writes > 0);
    assert_eq!(composition.presentation_id, face.identity);
    let show = service.show().unwrap().clone();
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    assert!(service.route_keyboard().unwrap());
    let writes = display.writes;
    let tab = KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(matches!(
        service.key(tab, 1, &mut display).unwrap(),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert!(display.writes > writes);
    assert_eq!(service.show().unwrap().show_id, show.show_id);
    let action = face
        .actions
        .iter()
        .position(|action| action.identity == "creche.suggest")
        .unwrap();
    let input = service
        .scene()
        .unwrap()
        .interaction(
            &face.identity,
            face.revision,
            crate::native_face_scene::FaceControl {
                action,
                argument: None,
            },
            &show,
            vec![],
            1,
        )
        .unwrap();
    let correlation = service.submit(input.clone()).unwrap();
    assert_eq!(correlation.show_id, show.show_id);
    assert_eq!(correlation.interaction, input);
    assert!(service.show().is_none());
    assert!(matches!(
        draft.apply_face_interaction(&basis, &show, &input),
        Ok(conduit_birth_plot::BirthActionOutcome::Changed)
    ));
    let next = draft.face(&basis).unwrap();
    assert_ne!(next.revision, face.revision);
    service.present(next, 2, 2, &mut display).unwrap();
    assert!(service.show().is_some());
}

#[test]
fn failed_scanout_does_not_publish_a_mask_show() {
    let (mut service, draft) = fixture();
    let mut display = CountingDisplay {
        writes: 0,
        fail: true,
    };
    let face = draft.face(&service.birth_basis("arrival/two")).unwrap();
    assert!(matches!(
        service.present(face, 1, 1, &mut display),
        Err(NativeFaceMaskError::Compositor(_))
    ));
    assert!(service.show().is_none());
}
