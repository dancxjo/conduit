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
    let plan = &prepared.body_plan.plots[0].plan;
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
    state: &std::path::Path,
) -> Result<bool> {
    checked(face.publish(
        PublicationSnapshot {
            sequence,
            channel,
            canonical: bytes,
        },
        committed_ms,
    ))?;
    let current = face.current();
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
