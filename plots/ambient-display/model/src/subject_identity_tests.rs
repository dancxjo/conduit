//! Identity parity proof only; no prospective reservation or publication.
use super::*;
use crate::PreparedAmbientFaces;
use conduit_core::StructuredInfoValue;
use conduit_face::{FaceFragment, FaceRole};

#[path = "tests/fixtures.rs"]
mod fixtures;
use fixtures::*;

fn semantic_subject<'a>(fragment: &'a FaceFragment, role: &str) -> &'a str {
    fragment
        .subjects
        .iter()
        .find(|subject| matches!(&subject.role, FaceRole::Semantic(kind) if kind.as_str() == role))
        .unwrap()
        .identity
        .as_str()
}

fn assert_refs(refs: &AmbientClockSubjectRefs, clock: &FaceFragment, image: &FaceFragment) {
    assert_eq!(
        refs.clock,
        semantic_subject(clock, "time/civil-observation")
    );
    assert_eq!(refs.weather, semantic_subject(clock, "weather/conditions"));
    assert_eq!(refs.image, semantic_subject(image, "media/image"));
    for reference in [&refs.clock, &refs.weather, &refs.image] {
        assert_eq!(
            reference.capacity(),
            crate::projection_memory::IDENTITY_BYTES
        );
        assert!(reference.len() <= conduit_face::MAX_FACE_ID_BYTES);
    }
}

#[test]
fn prospective_subject_refs_equal_actual_home_and_separate_facet_projection() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let picture = observed_wallpaper(&prepared, "available", 301_000);
    let current = clock(&prepared, "fresh", 150, 150);
    let home_basis = basis();
    let refs = ambient_clock_subject_refs(&home_basis, &home_basis).unwrap();
    let home = prepared.home(home_basis, &picture, &current).unwrap();
    assert_refs(&refs, &home, &home);
    face(home); // The projected Home also retains the original Face grammar.

    let mut clock_basis = basis();
    clock_basis.checked_plot_id = "plot/separate-clock".into();
    let mut image_basis = basis();
    image_basis.checked_plot_id = "plot/separate-wallpaper".into();
    let refs = ambient_clock_subject_refs(&clock_basis, &image_basis).unwrap();
    let clock = prepared.clock_weather(clock_basis, &current).unwrap();
    let image = prepared
        .wallpaper_observation(image_basis, &picture)
        .unwrap();
    assert_refs(&refs, &clock, &image);
}

#[test]
fn each_scoped_basis_identity_changes_refs_and_invalid_basis_refuses() {
    let original = basis();
    let refs = ambient_clock_subject_refs(&original, &original).unwrap();
    for field in 0..3 {
        let mut changed = original.clone();
        match field {
            0 => changed.checked_plot_id = "plot/other".into(),
            1 => changed.plan_id = "plan/other".into(),
            _ => changed.active_play_id = "play/other".into(),
        }
        let new = ambient_clock_subject_refs(&changed, &changed).unwrap();
        assert_ne!(refs.clock, new.clock);
        assert_ne!(refs.weather, new.weather);
        assert_ne!(refs.image, new.image);
        for value in [
            String::new(),
            "x".repeat(conduit_face::MAX_FACE_ID_BYTES + 1),
        ] {
            let mut invalid = original.clone();
            match field {
                0 => invalid.checked_plot_id = value.clone().into(),
                1 => invalid.plan_id = value.clone().into(),
                _ => invalid.active_play_id = value.clone().into(),
            }
            assert_eq!(
                ambient_clock_subject_refs(&invalid, &original),
                Err(AmbientFaceRefusal::InvalidBasis)
            );
            assert_eq!(
                ambient_clock_subject_refs(&original, &invalid),
                Err(AmbientFaceRefusal::InvalidBasis)
            );
        }
    }
}

#[test]
fn extracted_identity_keeps_original_digest_bytes_and_child_capacity() {
    let basis = basis();
    for role in ["clock-weather", "wallpaper"] {
        let digest = conduit_core::semantic_digest(
            "ambient-face-subject@1",
            format!("{:?}", (&basis, role)).as_bytes(),
        );
        let suffix: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        let old = crate::projection_memory::subject_identity(&suffix, role);
        let extracted = subject_root(&basis, role).unwrap();
        assert_eq!(old, extracted);
        assert_eq!(old.capacity(), extracted.capacity());
    }
}
