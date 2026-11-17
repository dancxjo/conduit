//! Shared cache proof with no native Mask, SDL, network or received authority.
use super::*;
use crate::ambient_wallpaper::{acquisition::*, MAXIMUM_APOD_RESPONSE_BYTES};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use std::os::unix::fs::PermissionsExt;

fn acquired() -> PreparedDeck {
    let admission = PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0;
    prepare_deck(admission,|url,bound| {
        if bound==MAXIMUM_APOD_RESPONSE_BYTES {
            Ok(serde_json::to_vec(&serde_json::json!({"date":url.rsplit('=').next().unwrap(),"title":"Actual source title","media_type":"image","hdurl":"https://apod.nasa.gov/fixture.png","copyright":"Exact photographer credit"})).unwrap())
        } else {
            number+=1;
            let mut bytes=Vec::new();
            PngEncoder::new(&mut bytes).write_image(&[number,2,3,255],1,1,ExtendedColorType::Rgba8).unwrap();
            Ok(bytes)
        }
    }).unwrap()
}
fn cache() -> (tempfile::TempDir, std::path::PathBuf, [u8; 32]) {
    let parent = tempfile::tempdir().unwrap();
    let directory = parent.path().join("photos");
    let checksum = write_deck(&directory, &acquired()).unwrap();
    (parent, directory.join("manifest.json"), checksum)
}
fn rewrite(path: &Path, change: impl FnOnce(&mut serde_json::Value)) -> [u8; 32] {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    change(&mut manifest);
    let bytes = serde_json::to_vec(&manifest).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(path, &bytes).unwrap();
    Sha256::digest(&bytes).into()
}
#[test]
fn source_boot_reuses_the_identical_checked_deck_without_acquisition_or_decode() {
    let (_parent, path, checksum) = cache();
    let expected = acquired();
    for _ in 0..2 {
        let cached = load_deck(&path, checksum).unwrap();
        for (a, b) in cached.photos().iter().zip(expected.photos()) {
            assert_eq!(a.candidate(), b.candidate());
            assert_eq!(a.rgba(), b.rgba());
            assert_eq!(a.original_sha256(), b.original_sha256());
            assert_eq!(a.version_sha256(), b.version_sha256());
            assert_eq!((a.width(), a.height()), (b.width(), b.height()));
            assert_eq!(a.license(), None);
        }
        let owned: usize = cached
            .photos()
            .iter()
            .map(|p| {
                p.rgba().len()
                    + p.candidate().date.capacity()
                    + p.candidate().title.capacity()
                    + p.candidate().asset_url.capacity()
                    + p.candidate().credit.as_ref().map_or(0, String::capacity)
            })
            .sum();
        assert_eq!(cached.retained_bytes(), owned);
        assert!(owned <= MAXIMUM_RETAINED_BYTES);
    }
    assert!(load_deck(&path, [99; 32]).is_err());
}
#[test]
fn source_requires_all_provenance_but_native_data_only_cache_stays_valid() {
    let (_parent, path, _) = cache();
    let checksum = rewrite(&path, |m| {
        m["photos"][0].as_object_mut().unwrap().remove("provenance");
    });
    assert!(load_photos(&path, checksum).is_ok());
    assert!(matches!(load_deck(&path,checksum),Err(error) if error.contains("provenance")));
}
#[test]
fn invalid_actual_provenance_is_never_guessed_or_truncated() {
    for (key, value) in [
        ("credit", serde_json::Value::String("x".repeat(257))),
        (
            "asset_url",
            serde_json::json!("https://unoffered.test/photo.jpg"),
        ),
        ("date", serde_json::json!("2026-02-29")),
    ] {
        let (_parent, path, _) = cache();
        let checksum = rewrite(&path, |m| m["photos"][0]["provenance"][key] = value);
        assert!(load_deck(&path, checksum).is_err());
    }
}
#[test]
fn exact_file_versions_bounds_opacity_paths_and_no_follow_are_checked() {
    for change in ["version", "extent", "path", "duplicate"] {
        let (_parent, path, _) = cache();
        let checksum = rewrite(&path, |m| match change {
            "version" => m["photos"][0]["version_sha256"][0] = serde_json::json!(99),
            "extent" => m["photos"][0]["width"] = serde_json::json!(1025),
            "path" => m["photos"][0]["file"] = serde_json::json!("../outside.rgba"),
            "duplicate" => m["photos"][1] = m["photos"][0].clone(),
            _ => unreachable!(),
        });
        assert!(load_deck(&path, checksum).is_err());
    }
    let (_parent, path, checksum) = cache();
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let image = path.parent().unwrap().join(&manifest.photos[0].file);
    std::fs::remove_file(&image).unwrap();
    std::os::unix::fs::symlink(
        path.parent().unwrap().join(&manifest.photos[1].file),
        &image,
    )
    .unwrap();
    assert!(load_deck(&path, checksum).is_err());
    let (_parent, path, _) = cache();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(&path, vec![0; MAXIMUM_MANIFEST_BYTES + 1]).unwrap();
    assert!(load_deck(&path, [1; 32]).is_err());
}
