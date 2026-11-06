//! View aliases retain their original owner without gaining interaction rights.
use super::*;

#[test]
fn exported_face_and_clones_keep_original_custody_after_encounter_retirement() {
    let mut prepared = None;
    let mut directory = None;
    image_custody::with_verified_root(|root| {
        // SAFETY: same bounded immutable fixture as the input-custody tests;
        // synchronous preparation starts no worker and exports no input alias.
        prepared = Some(
            unsafe {
                CrecheEncounterFactory::prepare_in_image(
                    root,
                    4 * 1024 * 1024,
                    2 * 1024 * 1024,
                    || {
                        let (factory, files) = factory();
                        directory = Some(files);
                        Ok(factory)
                    },
                )
            }
            .unwrap(),
        );
    });
    let mut factory = prepared.unwrap();
    let InputsCustody::Image(inputs) = &factory.inputs else {
        panic!("funded inputs expected")
    };
    let original_custody = Arc::downgrade(inputs);
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
        .unwrap();
    start_fixture(&mut adapter);
    let view = controller.face().unwrap().unwrap();
    assert!(core::ptr::eq(
        &*view,
        controller
            .channel
            .lock()
            .unwrap()
            .face
            .as_ref()
            .unwrap()
            .as_ref(),
    ));
    let (alias, measured) = crate::allocation_probe::observe(|| view.clone());
    assert_eq!(measured.allocations, 0);
    assert!(core::ptr::eq(&*view, &*alias));
    let revision = view.revision;
    adapter.cancel_and_drain().unwrap();
    assert!(controller.face().unwrap().is_none());
    drop(adapter);
    drop(controller);
    drop(factory);
    assert!(original_custody.upgrade().is_some());
    assert_eq!(view.revision, revision);
    drop(view);
    assert!(original_custody.upgrade().is_some());
    assert_eq!(alias.revision, revision);
    drop(alias);
    assert!(original_custody.upgrade().is_none());
    drop(directory);
}

#[test]
fn retained_old_view_is_readable_but_cannot_refresh_a_stale_action() {
    let (mut factory, _directory) = factory();
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
        .unwrap();
    start_fixture(&mut adapter);
    let old = controller.face().unwrap().unwrap();
    let show = crate::mask_test_common::available_mask_show(&old);
    let input =
        FaceInteraction::new(&old, &show, "host-config/source", "host-config", vec![], 1).unwrap();
    controller.acknowledge_show(show.clone()).unwrap();
    controller.submit(input.clone()).unwrap();
    adapter.poll().unwrap();
    let current = controller.face().unwrap().unwrap();
    assert!(current.revision > old.revision);
    assert!(!core::ptr::eq(&*current, &*old));
    assert!(controller.acknowledge_show(show).is_err());
    assert!(controller.submit(input).is_err());
    adapter.cancel_and_drain().unwrap();
    assert!(!old.text.is_empty());
}
