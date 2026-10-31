//! Read-only geometry of source history already present in the shared Face.
//! No collection, polling, membership decision, or history storage is owned here.
use conduit_environment_history::{
    SourceSeriesStyle,
    source_series::{PreparedSourceSeries, SOURCE_SERIES_SCHEMA, SourceSeriesGraph},
};
use conduit_face::{
    Face, FacePropertyValue as V, FaceRole, GraphicsPaintRole as Paint, GraphicsScene,
};
pub struct Prepared {
    pub scene: GraphicsScene,
    pub label: String,
}
pub fn present(face: &Face) -> bool {
    face.properties.iter().any(|p| {
        p.name == "schema" && matches!(&p.value, V::Text(schema) if schema == SOURCE_SERIES_SCHEMA)
    })
}
fn requested(s: &PreparedSourceSeries) -> Option<SourceSeriesStyle> {
    // Explicit local presentation preference for the operator's four names.
    // This affects paint only; names never select source/equipment/authority.
    Some(SourceSeriesStyle {
        paint: match s.name.as_str() {
            "Blue" => Paint::Accent,
            "Green" => Paint::Success,
            "White" => Paint::Foreground,
            "Yellow" => Paint::Warning,
            _ => return None,
        },
        dashed: s.name == "White",
    })
}
pub fn prepare(
    face: &Face,
    unit: conduit_core::Unit,
    width: u16,
    height: u16,
    window: Option<u64>,
    look: Option<&conduit_face::LookInfo>,
) -> Result<Option<Prepared>, String> {
    let Some(graph) = parse(face)? else {
        return Ok(None);
    };
    let mut assignments: Vec<(&str, SourceSeriesStyle)> = vec![];
    let mut ordered: Vec<_> = graph.series().iter().collect();
    ordered.sort_unstable_by_key(|s| (requested(s).is_none(), s.source.as_str()));
    for s in ordered {
        let preferred = requested(s).filter(|style| {
            !assignments
                .iter()
                .any(|(_, used)| used.paint == style.paint)
        });
        let style = match preferred {
            Some(style) => style,
            None => {
                let paint = [
                    Paint::Accent,
                    Paint::Success,
                    Paint::Foreground,
                    Paint::Warning,
                ]
                .into_iter()
                .find(|p| !assignments.iter().any(|(_, style)| style.paint == *p))
                .ok_or("No distinct source style available")?;
                SourceSeriesStyle {
                    paint,
                    dashed: paint == Paint::Foreground,
                }
            }
        };
        assignments.push((&s.subject, style));
    }
    let scene = graph
        .native_scene_at_size(unit, width, height, window, |s| {
            assignments
                .iter()
                .find(|(id, _)| *id == s.subject)
                .expect("validated source style assignment")
                .1
        })
        .map_err(|e| format!("Source history drawing refused: {e:?}"))?;
    let typography = crate::raster_target::selected_typography(look)
        .map_err(|e| format!("Source chart typography refused: {e:?}"))?;
    for command in scene
        .commands()
        .iter()
        .filter(|c| c.kind == conduit_face::GraphicsCommandKind::Text)
    {
        let measured = conduitos::display::typography::TextLayout::new_selected(
            command.payload(),
            typography.style(command.text_role().into()),
            command.bounds.width,
            100,
        )
        .map_err(|_| "Source chart typography refused")?
        .finish_height();
        if measured > u32::from(command.bounds.height) {
            return Err("Source chart label does not fit; inspect Details".into());
        }
    }
    let mut host_names = face.subjects.iter().filter(|s| {
        s.role == FaceRole::Host
            && face.properties.iter().any(|p| {
                p.subject == s.identity
                    && p.name == "host-id"
                    && p.value == V::Identity(graph.collector_host().into())
            })
            && face.properties.iter().any(|p| {
                p.subject == s.identity
                    && p.name == "boot-id"
                    && p.value == V::Identity(graph.collector_boot().into())
            })
    });
    let label = match (host_names.next(), host_names.next()) {
        (Some(host), None) if !host.name.trim().is_empty() && host.name.len() <= 48 => {
            format!("Observed by {}", host.name)
        }
        _ => "Collector observations · Details for source".into(),
    };
    Ok(Some(Prepared { scene, label }))
}
pub fn refused_scene(reason: &str, width: u16, height: u16) -> Result<GraphicsScene, String> {
    use conduit_face::{GraphicsCommand as C, GraphicsShapeStyle as S, LayoutRect as R};
    let clip = R {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(C::rect(clip, clip, Paint::Background, S::Fill).map_err(|e| format!("{e:?}"))?)
        .map_err(|e| format!("{e:?}"))?;
    for (y, text) in [
        (48, reason),
        (112, "Details retains the original shared Face"),
    ] {
        scene
            .push(
                C::text(
                    R {
                        x: 16,
                        y,
                        width: width.saturating_sub(32),
                        height: 56,
                    },
                    clip,
                    Paint::Foreground,
                    text,
                )
                .map_err(|e| format!("{e:?}"))?,
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    Ok(scene)
}

fn parse(face: &Face) -> Result<Option<SourceSeriesGraph>, String> {
    let mut collections = face.properties.iter().filter(|p| {
        p.name == "schema" && matches!(&p.value, V::Text(schema) if schema == SOURCE_SERIES_SCHEMA)
    });
    let Some(collection) = collections.next() else {
        return Ok(None);
    };
    if collections.next().is_some() {
        return Err("Separate collector scopes; inspect Details".into());
    }
    let graph = SourceSeriesGraph::prepare(face, &collection.subject)
        .map_err(|e| format!("Source history refused: {e:?}"))?;
    Ok(Some(graph))
}

/// Only the validated profile's own technical subjects are summarized by its graph.
pub fn scope(face: &Face) -> Result<Vec<String>, String> {
    let Some(graph) = parse(face)? else {
        return Ok(vec![]);
    };
    let mut subjects = vec![graph.subject().to_string()];
    for series in graph.series() {
        subjects.push(series.subject.clone());
        subjects.extend(
            face.relationships
                .iter()
                .filter(|r| {
                    r.source == series.subject
                        && r.kind == conduit_face::FaceRelationshipKind::Contains
                })
                .map(|r| r.target.clone()),
        );
    }
    Ok(subjects)
}
#[cfg(test)]
#[path = "source_history_tests.rs"]
mod tests;
