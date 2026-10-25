//! The verified publication's existing semantic Face, then a pure Mask/Look.
use crate::{checked, planning::Prepared, Result};
use conduit_environment_runtime::{
    publication::{PublicationChannel, PublicationSnapshot},
    retained_publication_face::RetainedPublicationFace,
};
use conduit_face::{FaceBasis, FacePropertyValue};
pub fn age(face: &mut RetainedPublicationFace, now: u64, state: &std::path::Path) -> Result<()> {
    let revision = face.current().revision;
    let current = checked(face.at(now))?;
    if current.revision != revision {
        serde_json::to_writer(
            std::fs::File::create(state.join("face-current.json"))?,
            current,
        )?;
    }
    Ok(())
}
pub fn prepare(prepared: &Prepared) -> Result<RetainedPublicationFace> {
    let plan = &prepared
        .body_plan
        .plots
        .iter()
        .find(|p| p.plan.plan_id == prepared.active.plan_id)
        .ok_or("sampler partition")?
        .plan;
    checked(RetainedPublicationFace::prepare(
        FaceBasis {
            body_id: Some(prepared.body_plan.body_id.clone()),
            wake_id: Some(prepared.body_plan.wake_id.clone()),
            source_document_id: Some(plan.source_document_id.clone()),
            checked_plot_id: Some(plan.checked_plot_id.clone()),
            expanded_plot_id: Some(plan.expanded_plot_id.clone()),
            plan_id: Some(prepared.active.plan_id.clone()),
            active_play_id: Some(prepared.active.active_play_id.clone()),
            sign_ids: vec![],
        },
        960,
        120000,
    ))
}
pub fn publish(
    face: &mut RetainedPublicationFace,
    sequence: u64,
    channel: PublicationChannel,
    bytes: &[u8],
    committed_ms: u64,
    observed_ms: u64,
    state: &std::path::Path,
) -> Result<bool> {
    let current = commit_observed(
        face,
        PublicationSnapshot {
            sequence,
            channel,
            canonical: bytes,
        },
        committed_ms,
        observed_ms,
    )?;
    serde_json::to_writer(
        std::fs::File::create(state.join("face-current.json"))?,
        current,
    )?;
    let projection = checked(
        conduit_seven_segment_mask::PreparedEnvironmentFace::prepare(current, "environment"),
    )?;
    for selection in [
        conduit_seven_segment_mask::SevenSegmentSelection::Temperature,
        conduit_seven_segment_mask::SevenSegmentSelection::Humidity,
    ] {
        let show = conduit_seven_segment_mask::render(
            &projection,
            selection,
            &conduit_seven_segment_mask::SevenSegmentLook::fahrenheit().with_colon_fraction(),
        );
        println!(
            "{}",
            serde_json::json!({"event":"seven-segment-mask-preview","physical_write":false,"face":show.face_content_id,
            "revision":show.face_revision,"disposition":format!("{:?}",show.disposition),
            "digits":show.show.digits().map(|p|conduit_seven_segment_mask::SegmentPatternForm::encode(p)[0]),
            "decimal_points":show.show.decimal_points(),"colon":show.show.colon()})
        );
    }
    Ok(current.properties.iter().any(|p| {
        p.name == "sensor-status" && matches!(&p.value,FacePropertyValue::Text(t) if t=="available")
    }))
}

fn commit_observed<'a>(
    face: &'a mut RetainedPublicationFace,
    snapshot: PublicationSnapshot<'_>,
    committed_ms: u64,
    observed_ms: u64,
) -> Result<&'a conduit_face::Face> {
    if observed_ms < committed_ms {
        return Err("observation precedes its commit clock".into());
    }
    checked(face.publish(snapshot, committed_ms))?;
    // Both clocks are signed in the same MCU Boot domain. Age belongs to the
    // Face; a delayed carrier does not erase an otherwise valid measurement.
    checked(face.at(observed_ms))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_measurement_uses_face_age_policy_and_clock_reversal_preserves_sequence() {
        use conduit_environment_runtime::observation::{
            ObservationContract, PreparedSourceFrameDecoder,
        };
        let contract = ObservationContract::prepare().unwrap();
        let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
        let bytes = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
        let mut face = RetainedPublicationFace::prepare(
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: Some("fixture/source".into()),
                checked_plot_id: Some("fixture/checked".into()),
                expanded_plot_id: Some("fixture/expanded".into()),
                plan_id: Some("fixture/plan".into()),
                active_play_id: Some("fixture/play".into()),
                sign_ids: vec![],
            },
            960,
            120000,
        )
        .unwrap();
        let snapshot = |sequence| PublicationSnapshot {
            sequence,
            channel: PublicationChannel::Reading,
            canonical: bytes,
        };
        let fresh = |face: &conduit_face::Face| {
            face.properties
                .iter()
                .any(|p| p.name == "fresh" && matches!(p.value, FacePropertyValue::Flag(true)))
        };
        assert!(fresh(
            commit_observed(&mut face, snapshot(1), 1000, 11000).unwrap()
        ));
        assert!(!fresh(face.at(121001).unwrap()));
        assert!(fresh(
            commit_observed(&mut face, snapshot(2), 125000, 125100).unwrap()
        ));
        assert!(commit_observed(&mut face, snapshot(3), 126000, 125999).is_err());
        assert_eq!(face.last_source_sequence(), 2);
    }
}
