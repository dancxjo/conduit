use super::*;
use conduit_face::{GraphicsCommand, GraphicsPaintRole, LayoutRect};
fn show_entry() -> conduit_core::BaseProviderEntry {
    use conduit_core::{BaseEnforcementClass, BaseLifecycle, BaseProviderEntry, ResourceOffer};
    BaseProviderEntry {
        base_id: "linux/test/display".into(),
        provider_instance_id: "linux/test/display/1".into(),
        provider_generation: 1,
        implementation_id: "linux/test/sdl@1".into(),
        mechanism_family: "linux/test/display@1".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![ResourceOffer {
            pool_id: "linux/test/display/surface".into(),
            class_id: conduit_face::SHOW_RESOURCE_CLASS.into(),
            capacity_units: 1,
            compute: None,
            content: None,
        }],
    }
}
fn scene() -> GraphicsScene {
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 32,
        height: 24,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, "67.6F").unwrap(),
        )
        .unwrap();
    scene
}
#[test]
fn finite_surface_uses_ordinary_graphics_and_revocation_erases_pixels() {
    for (w, h) in [(0, 24), (32, 0), (u32::MAX, 24), (4096, 4096)] {
        assert!(Surface::prepare(w, h).is_err());
    }
    let mut surface = Surface::prepare(32, 24).unwrap();
    assert_eq!(surface.bytes().len(), 32 * 24 * 4);
    let before = surface.bytes().as_ptr();
    let receipt = surface.render(&scene()).unwrap();
    assert_eq!(receipt.commands, 1);
    assert!(receipt.pixels_written > 0);
    assert!(surface.bytes().iter().any(|b| *b != 0));
    surface.render(&GraphicsScene::empty()).unwrap();
    assert!(surface
        .bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| *p == 0xff00_0000_u32.to_ne_bytes()));
    assert_eq!(surface.bytes().as_ptr(), before);
    surface.lose();
    assert_eq!(surface.render(&scene()), Err("display-custody-lost"));
    assert!(surface.bytes().iter().all(|b| *b == 0));
}
#[test]
fn actual_sdl_software_texture_accepts_scene_and_refuses_after_loss() {
    // Explicit CPU diagnostic backend; this makes no DRM/HDMI acceptance claim.
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    assert_eq!(video.current_video_driver(), "dummy");
    let window = video
        .window("CPU fixture", 32, 24)
        .hidden()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut scanout = Scanout::prepare(&mut canvas, &textures, 32, 24).unwrap();
    let initialized = scanout.initialized_display().unwrap();
    let retained = initialized.clone();
    assert!(initialized.same_instance(&retained));
    assert!(scanout.matches_initialized(&initialized));
    assert!(initialized.is_live());
    assert_eq!(initialized.backend(), "dummy");
    assert_eq!(initialized.renderer(), "software");
    assert!(!initialized.is_kmsdrm());
    assert_eq!((initialized.width(), initialized.height()), (32, 24));
    assert_eq!(initialized.window_size(), (32, 24));
    assert_eq!(initialized.window_id(), scanout.canvas.window().id());
    assert!(initialized.instance_nonce() > 0);
    let host_claim = initialized.claim_host().unwrap();
    assert_eq!(
        host_claim.initialized().instance_nonce(),
        initialized.instance_nonce()
    );
    assert!(matches!(
        retained.claim_host(),
        Err(DisplayClaimRefusal::AlreadyClaimed)
    ));
    scanout.present(&scene()).unwrap();
    let pixels = scanout
        .canvas
        .read_pixels(None, PixelFormatEnum::ARGB8888)
        .unwrap();
    assert_eq!(pixels, scanout.surface.bytes());
    let backing = scanout.surface.bytes().as_ptr();
    let format = scanout.format();
    // Write a marker distinct from the already-presented frame through the
    // compositor's PixelTarget entrance. No SDL presentation happens yet.
    let prior = u32::from_ne_bytes(scanout.surface.bytes()[..4].try_into().unwrap());
    let marker = prior ^ 0x00ff_ffff;
    scanout.write_pixel(0, 0, marker).unwrap();
    let before_commit = scanout
        .canvas
        .read_pixels(None, PixelFormatEnum::ARGB8888)
        .unwrap();
    assert_eq!(
        u32::from_ne_bytes(before_commit[..4].try_into().unwrap()),
        prior
    );
    assert_ne!(before_commit, scanout.surface.bytes());
    // Possession validation must not commit the newly written marker. The
    // exact Ready Show entry remains descriptive until an ordinary Mask Show.
    let provider = conduitos::product_bases::NativeSurfaceProvider::from_initialized_output(
        show_entry(),
        [7; 32],
        &mut scanout,
    )
    .unwrap();
    assert_eq!(provider.entry, show_entry());
    assert_eq!(
        scanout
            .canvas
            .read_pixels(None, PixelFormatEnum::ARGB8888)
            .unwrap(),
        before_commit
    );
    PixelTarget::commit_frame(&mut scanout).unwrap();
    let after_commit = scanout
        .canvas
        .read_pixels(None, PixelFormatEnum::ARGB8888)
        .unwrap();
    assert_eq!(after_commit, scanout.surface.bytes());
    assert_eq!(
        scanout.surface.bytes().as_ptr(),
        backing,
        "commits retain the admitted single raster"
    );
    crate::input_queue_tests::live_events(&mut scanout, &sdl);
    // Invalid output is terminal; later commit cannot publish partial pixels.
    assert_eq!(
        scanout.write_pixel(format.width, 0, 0),
        Err(DisplayError::InvalidExtent)
    );
    assert!(!scanout.live);
    assert!(!scanout.matches_initialized(&initialized));
    assert!(
        conduitos::product_bases::NativeSurfaceProvider::from_initialized_output(
            show_entry(),
            [7; 32],
            &mut scanout,
        )
        .is_err()
    );
    assert!(!initialized.is_live() && !retained.is_live());
    assert!(scanout.initialized_display().is_err());
    assert_eq!(
        PixelTarget::commit_frame(&mut scanout),
        Err(DisplayError::Lost)
    );
    assert!(scanout.surface.bytes().iter().all(|byte| *byte == 0));
    scanout.lose();
    assert_eq!(
        scanout.present(&scene()),
        Err("display-custody-lost".into())
    );
    drop(host_claim);
    assert!(matches!(
        retained.claim_host(),
        Err(DisplayClaimRefusal::Lost)
    ));
    drop(scanout);
    // Repreparation on the same actual window is a fresh lifetime, not revival.
    let mut fresh = Scanout::prepare(&mut canvas, &textures, 32, 24).unwrap();
    let fresh_token = fresh.initialized_display().unwrap();
    assert_eq!(fresh_token.window_id(), retained.window_id());
    assert_ne!(fresh_token.instance_nonce(), retained.instance_nonce());
    assert!(fresh_token.is_live());
    assert!(!fresh_token.same_instance(&retained));
    assert!(!fresh.matches_initialized(&retained));
    assert!(fresh.matches_initialized(&fresh_token));
    assert!(!retained.is_live());
    let fresh_claim = fresh_token.claim_host().unwrap();
    drop(fresh_claim);
    assert!(!fresh_token.is_live());
    assert!(!fresh.matches_initialized(&fresh_token));
    assert!(
        conduitos::product_bases::NativeSurfaceProvider::from_initialized_output(
            show_entry(),
            [7; 32],
            &mut fresh,
        )
        .is_err()
    );
    assert_eq!(fresh.write_pixel(0, 0, 1), Err(DisplayError::Lost));
    assert_eq!(fresh.commit_frame(), Err(DisplayError::Lost));
    assert!(fresh.initialized_display().is_err());
    assert!(fresh.present(&scene()).is_err());
    drop(fresh);
    let mut next = Scanout::prepare(&mut canvas, &textures, 32, 24).unwrap();
    let next_token = next.initialized_display().unwrap();
    assert_ne!(next_token.instance_nonce(), fresh_token.instance_nonce());
    let next_claim = next_token.claim_host().unwrap();
    crate::input_queue_tests::focus_loss(&mut next, &sdl);
    drop(next);
    assert!(!next_claim.initialized().is_live());
    drop(next_claim);
    assert!(
        !fresh_token.is_live(),
        "Drop revokes all outstanding clones"
    );
}

#[test]
fn actual_sdl_kmsdrm_name_is_accepted_without_accepting_desktop_fallbacks() {
    // Catalogue inspection initializes no SDL subsystem or display and creates
    // no initialized-display token. This is an SDK identity boundary test.
    let reported = sdl2::video::drivers()
        .find(|name| name.eq_ignore_ascii_case("kmsdrm"))
        .expect("the native Linux test SDK must include its KMSDRM backend");
    assert!(is_kmsdrm_backend(reported));
    for name in sdl2::video::drivers().filter(|name| *name != reported) {
        assert!(
            !is_kmsdrm_backend(name),
            "fallback {name} is not direct scanout"
        );
    }
}
