//! CPU possession/cache tests, not installed-Host or physical-display proof.
use super::*;
use conduitos::{
    display::{DisplayFormat, PixelTarget},
    product_bases::InitializedNativeOutput,
};

fn data() -> NativeMaskImageData {
    NativeMaskImageData::finish(
        (1..=4)
            .map(|n| {
                let rgba = vec![n, 2, 3, 255].into_boxed_slice();
                photo([n; 32], version(&[n; 32], 1, 1, &rgba), 1, 1, rgba).unwrap()
            })
            .collect(),
    )
    .unwrap()
}
struct Cpu;
impl PixelTarget for Cpu {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 8,
            height: 8,
            pitch: 32,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        Ok(())
    }
    fn commit_frame(&mut self) -> Result<(), DisplayError> {
        Ok(())
    }
}
impl InitializedNativeOutput for Cpu {
    fn require_live(&self) -> Result<(), DisplayError> {
        Ok(())
    }
}
fn provider() -> NativeSurfaceProvider {
    let entry = BaseProviderEntry {
        base_id: "fixture/display".into(),
        provider_instance_id: "fixture/display/1".into(),
        provider_generation: 1,
        implementation_id: "fixture/cpu@1".into(),
        mechanism_family: "fixture/display@1".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![resource_offer(
            "fixture/show",
            conduit_face::SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    NativeSurfaceProvider::from_initialized_output(entry, [9; 32], &mut Cpu).unwrap()
}
fn body(sequence: u64) -> BodyId {
    conduit_body::Body::born(
        "source/fixture".into(),
        "plot/fixture".into(),
        sequence,
        "fixture/born".into(),
    )
    .unwrap()
    .body_id
}
#[test]
fn actual_authorized_possession_and_exact_body_scope_gate_all_views() {
    let provider = provider();
    let offer = conduit_conduitos_mask_offer::native_read_only_host_advertisement(
        &"host/images".into(),
        &"boot/images".into(),
        1,
        &provider.entry,
        NativeMaskStorageProfile::Standard,
    );
    let stage = conduit_conduitos_mask_offer::prepare_stage_from_offer(
        conduit_conduitos_mask_offer::Adapter::Native,
        &offer,
    )
    .unwrap();
    let mut possession = ActiveNativeSurfacePossession::begin(
        &provider,
        &stage.planned_mask.plan,
        "play/images".into(),
    )
    .unwrap();
    let scope = ImageScope {
        body: body(1),
        host: offer.host_id,
        boot: offer.boot_id,
        provider: provider.entry.provider_instance_id.clone(),
        generation: 1,
    };
    let data = data();
    assert!(matches!(
        scope.views(&data, Some(&scope.body), &possession),
        Err(DisplayError::ImageAccessDenied)
    ));
    possession.authorize().unwrap();
    assert!(matches!(
        scope.views(&data, None, &possession),
        Err(DisplayError::ImageAccessDenied)
    ));
    assert!(matches!(
        scope.views(&data, Some(&body(2)), &possession),
        Err(DisplayError::ImageAccessDenied)
    ));
    let wrong_boot = ImageScope {
        body: scope.body.clone(),
        host: scope.host.clone(),
        boot: "boot/other".into(),
        provider: scope.provider.clone(),
        generation: 1,
    };
    assert!(matches!(
        wrong_boot.views(&data, Some(&scope.body), &possession),
        Err(DisplayError::ImageAccessDenied)
    ));
    let mut scene = conduit_face::GraphicsScene::empty();
    let bounds = conduit_face::LayoutRect {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
    };
    scene
        .push(conduit_face::GraphicsCommand::image(bounds, bounds, data.photos[0].key).unwrap())
        .unwrap();
    let probe = crate::allocation_probe::begin();
    let views = scope.views(&data, Some(&scope.body), &possession).unwrap();
    for p in &data.photos {
        assert!(views.resolve(p.key).is_ok());
    }
    let rendered = conduitos::display::render_scene_with_images(&mut Cpu, &scene, &views).unwrap();
    assert_eq!(rendered.commands, 1);
    assert!(matches!(
        views.resolve(GraphicsImageKey::new([1; 32], [99; 32]).unwrap()),
        Err(DisplayError::ImageStale)
    ));
    assert!(matches!(
        views.resolve(GraphicsImageKey::new([99; 32], [99; 32]).unwrap()),
        Err(DisplayError::ImageUnavailable)
    ));
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
    drop(views);
    assert_eq!(
        possession.complete_and_retire().unwrap().lifecycle,
        "revoked-at-play-end"
    );
}
#[test]
fn refuses_transparency_wrong_version_oversize_and_duplicate_generations() {
    assert!(photo([1; 32], [1; 32], 1, 1, vec![0, 0, 0, 0].into()).is_err());
    assert!(photo([1; 32], [1; 32], 1, 1, vec![0, 0, 0, 255].into()).is_err());
    assert!(photo([1; 32], [1; 32], 1025, 1, vec![0; 4100].into()).is_err());
    assert!(NativeMaskImageData::finish(vec![]).is_err());
    assert!(NativeMaskImageData::finish(
        (0..4)
            .map(|_| {
                let rgba = vec![1, 2, 3, 255].into_boxed_slice();
                photo([1; 32], version(&[1; 32], 1, 1, &rgba), 1, 1, rgba).unwrap()
            })
            .collect()
    )
    .is_err());
}
fn cache() -> (tempfile::TempDir, std::path::PathBuf, [u8; 32]) {
    let directory = tempfile::tempdir().unwrap();
    let data = data();
    let mut entries = Vec::new();
    for (i, p) in data.photos.iter().enumerate() {
        let file = format!("photo{i}.rgba");
        std::fs::write(directory.path().join(&file), &p.rgba).unwrap();
        entries.push(serde_json::json!({"file":file,"width":p.width,"height":p.height,
            "original_sha256":p.key.identity(),"version_sha256":p.key.version(),"rgba_sha256":<[u8;32]>::from(Sha256::digest(&p.rgba))}));
    }
    let bytes = serde_json::to_vec(&serde_json::json!({"photos":entries})).unwrap();
    let path = directory.path().join("manifest.json");
    std::fs::write(&path, &bytes).unwrap();
    (directory, path, Sha256::digest(&bytes).into())
}
#[test]
fn real_cache_checks_exact_manifest_and_file_custody() {
    let (directory, path, digest) = cache();
    let loaded = NativeMaskImageData::load_cache(&path, digest).unwrap();
    assert_eq!(loaded.photos[0].key, data().photos[0].key);
    assert!(NativeMaskImageData::load_cache(&path, [99; 32]).is_err());
    std::fs::write(directory.path().join("photo0.rgba"), [1, 2, 3, 255, 0]).unwrap();
    assert!(NativeMaskImageData::load_cache(&path, digest).is_err());
    std::fs::remove_file(directory.path().join("photo0.rgba")).unwrap();
    std::os::unix::fs::symlink(
        directory.path().join("photo1.rgba"),
        directory.path().join("photo0.rgba"),
    )
    .unwrap();
    assert!(NativeMaskImageData::load_cache(&path, digest).is_err());
    std::fs::write(&path, vec![0; MAXIMUM_MANIFEST_BYTES + 1]).unwrap();
    assert!(NativeMaskImageData::load_cache(&path, digest).is_err());
}
#[test]
fn factory_bounds_refuse_without_running_factory() {
    let called = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = called.clone();
    // SAFETY: inert fixture closure, no filesystem or network authority.
    let result = unsafe {
        NativeMaskImageInitialization::root_authorized(body(1), 1, 2, move || {
            flag.set(true);
            Ok(data())
        })
    };
    assert!(result.is_err());
    assert!(!called.get());
}

#[test]
fn explicit_operator_export_round_trips_actual_prepared_deck_and_refuses_replacement() {
    use crate::ambient_wallpaper::{acquisition::*, MAXIMUM_APOD_RESPONSE_BYTES};
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
    let admission = PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0;
    let deck=prepare_deck(admission,|url,bound| {
        if bound==MAXIMUM_APOD_RESPONSE_BYTES {
            Ok(serde_json::to_vec(&serde_json::json!({"date":url.rsplit('=').next().unwrap(),"title":"Prepared fixture","media_type":"image","hdurl":"https://apod.nasa.gov/fixture.png"})).unwrap())
        } else {
            number+=1;
            let mut bytes=Vec::new();
            PngEncoder::new(&mut bytes).write_image(&[number,2,3,255],1,1,ExtendedColorType::Rgba8).unwrap();
            Ok(bytes)
        }
    }).unwrap();
    let parent = tempfile::tempdir().unwrap();
    let directory = parent.path().join("immutable-cache");
    let checksum = NativeMaskImageData::write_cache(&directory, &deck).unwrap();
    let loaded =
        NativeMaskImageData::load_cache(&directory.join("manifest.json"), checksum).unwrap();
    for (p, actual) in deck.photos().iter().zip(&loaded.photos) {
        assert_eq!(actual.rgba.as_ref(), p.rgba());
        assert_eq!(actual.key.identity(), *p.original_sha256());
        assert_eq!(actual.key.version(), *p.version_sha256());
    }
    assert!(NativeMaskImageData::write_cache(&directory, &deck).is_err());
}
