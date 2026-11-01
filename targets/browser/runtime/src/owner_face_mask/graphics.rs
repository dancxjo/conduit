//! Graphical leaves prepared from the exact typed Face already admitted by
//! OwnerBrowserMask. No network fetch, history store or execution authority.
use conduit_environment_history::{
    source_series::{SourceSeriesGraph, SOURCE_SERIES_SCHEMA},
    source_series_styles,
};
use conduit_face::{Face, FacePropertyValue, FaceSubject, GraphicsCommandKind, GraphicsScene};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct GraphicsView {
    pub width: u16,
    pub height: u16,
    pub commands: Vec<CommandView>,
    pub refusal: Option<String>,
}
#[derive(Serialize)]
pub(super) struct CommandView {
    kind: String,
    bounds: [i32; 4],
    clip: [i32; 4],
    paint: String,
    style: String,
    text_role: String,
    text: String,
    points: Vec<[i16; 2]>,
}
fn rect(r: conduit_face::LayoutRect) -> [i32; 4] {
    [
        i32::from(r.x),
        i32::from(r.y),
        i32::from(r.width),
        i32::from(r.height),
    ]
}
fn commands(scene: &GraphicsScene) -> Vec<CommandView> {
    scene
        .commands()
        .iter()
        .map(|c| CommandView {
            kind: format!("{:?}", c.kind),
            bounds: rect(c.bounds),
            clip: rect(c.clip),
            paint: format!("{:?}", c.paint),
            style: format!("{:?}", c.style),
            text_role: format!("{:?}", c.text_role()),
            text: c.payload().into(),
            points: match c.kind {
                GraphicsCommandKind::Polyline => c
                    .polyline_geometry()
                    .expect("validated polyline")
                    .points()
                    .iter()
                    .map(|p| [*p.x(), *p.y()])
                    .collect(),
                GraphicsCommandKind::OrthogonalPath => c
                    .path_geometry()
                    .expect("validated path")
                    .points()
                    .iter()
                    .map(|p| [*p.x(), *p.y()])
                    .collect(),
                _ => vec![],
            },
        })
        .collect()
}
pub(super) fn project(face: &Face, subject: &FaceSubject) -> Option<GraphicsView> {
    if !face.properties.iter().any(|p| {
        p.subject == subject.identity
            && p.name == "schema"
            && matches!(&p.value, FacePropertyValue::Text(s) if s == SOURCE_SERIES_SCHEMA)
    }) {
        return None;
    }
    // This finite viewport is a browser Mask realization choice, never Face truth.
    // The browser scrolls smaller surfaces without shrinking labels or resampling data.
    let mut view = GraphicsView {
        width: 960,
        height: 540,
        commands: vec![],
        refusal: None,
    };
    let prepared = SourceSeriesGraph::prepare(face, &subject.identity)
        .map_err(|e| format!("{e:?}"))
        .and_then(|graph| {
            let styles = source_series_styles(&graph);
            graph
                .native_scene_at_size(
                    conduit_core::Unit::Fahrenheit,
                    view.width,
                    view.height,
                    None,
                    |s| {
                        styles
                            .iter()
                            .find(|(id, _)| id == &s.subject)
                            .expect("complete style assignment")
                            .1
                    },
                )
                .map_err(|e| format!("{e:?}"))
        });
    match prepared {
        Ok(scene) => view.commands = commands(&scene),
        Err(reason) => {
            view.refusal = Some(format!(
                "History drawing unavailable: {reason}. Inspect exact facts."
            ))
        }
    }
    Some(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{Quantity, Unit};
    use conduit_environment_history::source_series::*;
    use conduit_environment_measurement::{
        source_observation::*, EnvironmentalReading, ReadingField,
    };
    use conduit_face::{FaceBasis, FaceContributionBasis};

    #[test]
    fn typed_face_history_reuses_shared_geometry_and_preserves_unavailable_samples() {
        let codec = ObservationContract::prepare().unwrap();
        let mut encoder = PreparedObservationCodec::prepare(&codec).unwrap();
        let mut observations = vec![];
        for (index, temperature) in [Some(20), Some(21), None, Some(22)].into_iter().enumerate() {
            let outcome = match temperature {
                Some(t) => SourceOutcome::Available(SourceSample {
                    source_counter: index as u64 + 1,
                    reported_source_ms: None,
                    reading: EnvironmentalReading::new(
                        Quantity::new(t, Unit::Celsius),
                        ReadingField::Available(Quantity::new(50, Unit::Percent)),
                        ReadingField::Unsupported,
                    )
                    .unwrap(),
                }),
                None => SourceOutcome::Unavailable(SourceFailure::InvalidSample),
            };
            observations.push(
                encoder
                    .encode(SourceObservation {
                        observed_at_host_boot_ms: 1000 + index as u64 * 1000,
                        outcome,
                    })
                    .unwrap()
                    .to_vec(),
            );
        }
        let records: Vec<_> = observations
            .iter()
            .enumerate()
            .map(|(i, bytes)| SourceSeriesRecord {
                sequence: i as u64 + 1,
                observation: bytes,
            })
            .collect();
        let series = [SourceSeries {
            subject: "series/blue",
            source: "equipment/blue",
            name: "Blue",
            samples_outside_view: 0,
            stale_after_ms: 5000,
            records: &records,
        }];
        let collection = SourceSeriesCollection {
            subject: "observations",
            collector_host: "host/collector",
            collector_boot: "boot/collector",
            observed_through_boot_ms: 4000,
            series: &series,
        };
        let fragment = collection
            .contribution(FaceContributionBasis {
                checked_plot_id: "checked/test".into(),
                plan_id: "plan/test".into(),
                active_play_id: "play/test".into(),
                required_interaction_context: None,
            })
            .unwrap();
        let face = Face::new(
            1,
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: Some("source/test".into()),
                checked_plot_id: Some("checked/test".into()),
                expanded_plot_id: Some("expanded/test".into()),
                plan_id: Some("plan/test".into()),
                active_play_id: Some("play/test".into()),
                sign_ids: vec![],
            },
            fragment.subjects,
            fragment.relationships,
            fragment.properties,
            fragment.text,
        )
        .unwrap();
        let graph = SourceSeriesGraph::prepare(&face, "observations").unwrap();
        let styles = source_series_styles(&graph);
        let expected = graph
            .native_scene_at_size(Unit::Fahrenheit, 960, 540, None, |s| {
                styles.iter().find(|(id, _)| id == &s.subject).unwrap().1
            })
            .unwrap();
        let view = project(&face, &face.subjects[0]).unwrap();
        assert!(view.refusal.is_none());
        assert_eq!(
            serde_json::to_value(&view.commands).unwrap(),
            serde_json::to_value(commands(&expected)).unwrap()
        );
        assert!(view.commands.len() <= 48);
        assert!(view.commands.iter().any(|c| c.kind == "Polyline"));
        assert!(view.commands.iter().any(|c| c.text.contains("°F")));
        let mut malformed = face.clone();
        let typed = malformed
            .properties
            .iter_mut()
            .find(|p| matches!(&p.value, FacePropertyValue::TypedValue { .. }))
            .unwrap();
        if let FacePropertyValue::TypedValue { bytes, .. } = &mut typed.value {
            bytes.clear();
        }
        let refused = project(&malformed, &malformed.subjects[0]).unwrap();
        assert!(refused.refusal.is_some());
        assert!(refused.commands.is_empty());
        assert_eq!(face.identity, malformed.identity); // no re-identification or fabricated samples
        assert!(project(&face, &face.subjects[1]).is_none());
        if let Ok(path) = std::env::var("CONDUIT_GRAPHICS_VIEW_FIXTURE") {
            std::fs::write(path, serde_json::to_vec(&view).unwrap()).unwrap();
        }
    }
}
