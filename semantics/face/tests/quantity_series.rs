//! Independent voltage observations, not environmental schemas or Body custody.
use conduit_core::{CheckedValueContract, Quantity, QUANTITY_ENCODED_LEN};
use conduit_face::{
    FaceProperty, FacePropertyValue as V, PreparedQuantitySeries as Series,
    QuantitySeriesRefusal as R,
};
#[path = "common/quantity_series.rs"]
mod common;
use common::fixture;
#[test]
fn irregular_voltage_times_gaps_units_and_unavailability_remain_exact() {
    let face = fixture();
    let before = face.clone();
    let series = Series::prepare(&face, "voltage", 3).unwrap();
    assert_eq!(
        series.producer(),
        ["host/meter", "boot/meter", "plan/meter", "play/meter"]
    );
    assert_eq!(series.omitted_samples(), 2);
    assert_eq!(
        (0..3)
            .map(|i| series.time_millionths(i).unwrap())
            .collect::<Vec<_>>(),
        [0, 100_000, 1_000_000]
    );
    assert_eq!(
        series.samples()[0].value,
        Some(Quantity::parse_plot_literal("10V").unwrap())
    );
    assert_eq!(series.samples()[1].value, None);
    assert_eq!(
        series.samples()[1].unavailable_reason,
        Some("meter-unavailable")
    );
    assert_eq!(series.samples()[2].gap_before, Some((3, 3)));
    assert_eq!(face, before);
}
#[test]
fn foreign_source_regressed_time_and_missing_gap_refuse_without_mutation() {
    for mode in 0..3 {
        let mut face = fixture();
        let property = match mode {
            0 => (
                "sample/2",
                "series-source",
                V::Identity("other-boot-source".into()),
            ),
            1 => ("sample/2", "sampled-at-boot-ms", V::Count(999)),
            _ => ("sample/4", "missing-sequence-first", V::Count(2)),
        };
        face.properties
            .iter_mut()
            .find(|p| p.subject == property.0 && p.name == property.1)
            .unwrap()
            .value = property.2;
        face.reseal_prepared().unwrap();
        let before = face.clone();
        let expected = [R::Provenance, R::Order, R::Gap][mode];
        assert!(matches!(Series::prepare(&face, "voltage", 3), Err(reason) if reason == expected));
        assert_eq!(face, before);
    }
}
#[test]
fn capacity_and_equal_times_are_explicit() {
    let mut face = fixture();
    assert!(matches!(
        Series::prepare(&face, "voltage", 2),
        Err(R::Capacity)
    ));
    for property in &mut face.properties {
        if property.name == "sampled-at-boot-ms" {
            property.value = V::Count(1000);
        }
    }
    face.reseal_prepared().unwrap();
    let series = Series::prepare(&face, "voltage", 3).unwrap();
    assert_eq!(
        (0..3)
            .map(|i| series.time_millionths(i).unwrap())
            .collect::<Vec<_>>(),
        [0, 0, 0]
    );
}

#[test]
fn inconsistent_family_availability_order_and_accounting_refuse_distinctly() {
    for (mode, expected) in [
        (0, R::Availability),
        (1, R::Quantity),
        (2, R::Order),
        (3, R::Provenance),
        (4, R::Availability),
        (5, R::Clock),
    ] {
        let mut face = fixture();
        let replacement = match mode {
            0 => ("sample/2", "series-available", V::Flag(true)),
            1 => (
                "sample/4",
                "voltage",
                V::TypedValue {
                    contract: CheckedValueContract::new(
                        conduit_core::TEMPERATURE_INFO_ID.into(),
                        QUANTITY_ENCODED_LEN as u32,
                        vec![],
                    )
                    .unwrap(),
                    bytes: Quantity::parse_plot_literal("12°C")
                        .unwrap()
                        .encode()
                        .to_vec(),
                },
            ),
            2 => ("sample/2", "sequence", V::Count(1)),
            3 => ("source", "visible-samples", V::Count(2)),
            4 => (
                "sample/1",
                "series-unavailability",
                V::Text("false-failure".into()),
            ),
            _ => ("source", "clock", V::Text("wall-clock-ms".into())),
        };
        if let Some(property) = face
            .properties
            .iter_mut()
            .find(|property| property.subject == replacement.0 && property.name == replacement.1)
        {
            property.value = replacement.2;
        } else {
            face.properties.push(FaceProperty {
                subject: replacement.0.into(),
                name: replacement.1.into(),
                value: replacement.2,
            });
        }
        face.reseal_prepared().unwrap();
        assert!(matches!(Series::prepare(&face, "voltage", 3), Err(reason) if reason == expected));
    }
}

#[test]
fn bounded_display_projection_keeps_real_time_and_reports_selection() {
    let face = fixture();
    let series = Series::prepare(&face, "voltage", 3).unwrap();
    let plot = series.project(None, 16).unwrap();
    assert_eq!(plot.points.len(), 2);
    assert_eq!(plot.unavailable_samples, 1);
    assert_eq!(plot.gap_runs, 1);
    assert_eq!(plot.minimum.as_deref(), Some("10 V"));
    assert_eq!(plot.maximum.as_deref(), Some("12 V"));
    assert_eq!(plot.points[0].value_millionths, Some(0));
    assert_eq!(plot.points[1].value_millionths, Some(1_000_000));
    assert_eq!(plot.points[0].time_millionths, 0);
    assert_eq!(plot.points[1].time_millionths, 1_000_000);
    let selected = series.project(None, 1).unwrap();
    assert_eq!(selected.omitted_points, 1);
    assert_eq!(selected.points[0].sequence, "4");
    assert!(series.project(None, 0).is_err());
    assert!(series
        .project(Some(conduit_core::Unit::Celsius), 16)
        .is_err());
}

#[test]
fn selected_units_label_converted_points_and_missing_values_never_become_zero() {
    let face = fixture();
    let series = Series::prepare(&face, "voltage", 3).unwrap();
    let unit = conduit_core::Unit::resolve("mV").unwrap();
    let plot = series.project(Some(unit), 16).unwrap();
    assert_eq!(plot.points[0].value.as_deref(), Some("10000 mV"));
    assert_eq!(plot.points[1].value.as_deref(), Some("12000 mV"));
    assert_eq!(plot.minimum.as_deref(), Some("10000 mV"));
    assert_eq!(plot.unavailable_samples, 1);
    assert_eq!(plot.points.len(), 2);
    let mut missing = fixture();
    missing
        .properties
        .retain(|property| property.name != "voltage");
    for property in &mut missing.properties {
        if property.name == "series-available" {
            property.value = V::Flag(false);
        }
    }
    missing.reseal_prepared().unwrap();
    let plot = Series::prepare(&missing, "voltage", 3)
        .unwrap()
        .project(None, 16)
        .unwrap();
    assert!(plot.points.is_empty());
    assert_eq!(plot.unavailable_samples, 3);
    assert!(plot.minimum.is_none() && plot.maximum.is_none() && plot.unit.is_none());
}

#[test]
fn duplicate_or_non_observation_source_members_cannot_inflate_omissions() {
    let mut face = fixture();
    let edge = face
        .relationships
        .iter()
        .find(|relation| relation.source == "source")
        .unwrap()
        .clone();
    face.relationships.push(edge);
    face.properties
        .iter_mut()
        .find(|property| property.name == "visible-samples")
        .unwrap()
        .value = V::Count(4);
    face.reseal_prepared().unwrap();
    assert!(matches!(
        Series::prepare(&face, "voltage", 3),
        Err(R::Duplicate)
    ));
    let mut face = fixture();
    face.relationships.push(conduit_face::FaceRelationship {
        source: "source".into(),
        target: "voltage".into(),
        kind: conduit_face::FaceRelationshipKind::Contains,
    });
    face.properties
        .iter_mut()
        .find(|property| property.name == "visible-samples")
        .unwrap()
        .value = V::Count(4);
    face.reseal_prepared().unwrap();
    assert!(matches!(
        Series::prepare(&face, "voltage", 3),
        Err(R::Provenance)
    ));
}
