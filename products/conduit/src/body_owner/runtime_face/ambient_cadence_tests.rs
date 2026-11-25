use super::*;
use conduit_face::{FaceContributionBasis, FaceSubject, FaceText};

fn sample(time: &str, observed: u64) -> FaceFragment {
    let property = |subject: &str, name: &str, value| FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    };
    let semantic = |identity: &str, role: &str| FaceSubject {
        identity: identity.into(),
        role: FaceRole::Semantic(role.into()),
        name: identity.into(),
    };
    FaceFragment {
        basis: FaceContributionBasis {
            checked_plot_id: "plot/ambient".into(),
            plan_id: "plan/ambient".into(),
            active_play_id: "play/ambient".into(),
            required_interaction_context: None,
        },
        subjects: vec![
            semantic("clock", "time/civil-observation"),
            semantic("weather", "weather/conditions"),
            semantic("photo", "media/image"),
        ],
        relationships: vec![],
        composition: vec![],
        properties: vec![
            property(
                "clock",
                "local-date",
                FacePropertyValue::Text("2026-11-25".into()),
            ),
            property("clock", "local-time", FacePropertyValue::Text(time.into())),
            property(
                "clock",
                "observed-at-unix-ms",
                FacePropertyValue::Count(observed),
            ),
            property(
                "clock",
                "utc-offset-minutes",
                FacePropertyValue::Signed(-480),
            ),
            property(
                "weather",
                "availability",
                FacePropertyValue::Text("current".into()),
            ),
            property(
                "weather",
                "reference-at-unix-ms",
                FacePropertyValue::Count(observed),
            ),
            property(
                "weather",
                "usable-until-unix-ms",
                FacePropertyValue::Count(90_000),
            ),
            property(
                "photo",
                "title",
                FacePropertyValue::Text("First photo".into()),
            ),
        ],
        text: vec![FaceText {
            subject: "clock".into(),
            text: format!("2026-11-25 · {time} · America/Los_Angeles (UTC−08:00)"),
        }],
        actions: vec![],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    }
}

#[test]
fn steady_ticks_preserve_the_current_minute_with_original_observation() {
    let old = sample("23:59:01", 1_000);
    let new = sample("23:59:59", 59_999);
    assert!(same_presented_minute(&old, &new));
    assert_ne!(old, new);
    assert!(!same_presented_minute(&new, &old));
}

#[test]
fn minute_midnight_and_offset_changes_are_new_presentations() {
    let old = sample("23:59:59", 59_999);
    let mut midnight = sample("00:00:00", 60_000);
    midnight.properties[0].value = FacePropertyValue::Text("2026-11-26".into());
    assert!(!same_presented_minute(&old, &midnight));

    let old = sample("01:00:01", 1_000);
    let mut changed_offset = sample("01:00:02", 2_000);
    changed_offset.properties[3].value = FacePropertyValue::Signed(-420);
    assert!(!same_presented_minute(&old, &changed_offset));
}

#[test]
fn photo_changes_weather_expiry_and_play_changes_are_visible_within_a_minute() {
    let old = sample("12:00:01", 1_000);
    let next = sample("12:00:02", 2_000);
    for (index, value) in [
        (4, FacePropertyValue::Text("stale".into())),
        (6, FacePropertyValue::Count(95_000)),
        (7, FacePropertyValue::Text("Second photo".into())),
    ] {
        let mut changed = next.clone();
        changed.properties[index].value = value;
        assert!(!same_presented_minute(&old, &changed), "property {index}");
    }
    let mut replaced = next;
    replaced.basis.active_play_id = "play/replacement".into();
    assert!(!same_presented_minute(&old, &replaced));
}
