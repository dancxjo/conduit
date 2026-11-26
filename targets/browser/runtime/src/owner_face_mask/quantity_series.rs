//! Ordinary checked Face Mask projection; no independent manifestation or owner.
use super::*;
use conduit_face::{
    FaceRole, PreparedQuantitySeries, ProjectedQuantitySeries, QUANTITY_SERIES_ROLE,
};
#[derive(Serialize)]
pub(super) struct SeriesView {
    plot: Option<ProjectedQuantitySeries>,
    refusal: Option<&'static str>,
}
pub(super) fn prepare(
    face: &Face,
    subject: &conduit_face::FaceSubject,
    accepted: &mut usize,
) -> Option<SeriesView> {
    if !matches!(&subject.role, FaceRole::Semantic(role) if role.as_str() == QUANTITY_SERIES_ROLE) {
        return None;
    }
    // Finite host presentation policy, separate from the complete Face reading.
    if *accepted == 8 {
        return Some(SeriesView {
            plot: None,
            refusal: Some("quantity-series-capacity"),
        });
    }
    let series = match PreparedQuantitySeries::prepare(face, &subject.identity, 64) {
        Ok(series) => series,
        Err(reason) => {
            return Some(SeriesView {
                plot: None,
                refusal: Some(reason.code()),
            });
        }
    };
    match series.project(None, 16) {
        Ok(plot) => {
            *accepted += 1;
            Some(SeriesView {
                plot: Some(plot),
                refusal: None,
            })
        }
        Err(reason) => Some(SeriesView {
            plot: None,
            refusal: Some(reason.code()),
        }),
    }
}

/// Optional plots never displace a complete reading that fitted the ABI before.
/// Count the exact encoded bytes without allocating a second serialized view.
pub(super) fn fit_view(view: &mut FaceView) {
    use std::io::{Error, Write};
    struct Budget(usize);
    impl Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| Error::other("view capacity"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    if !view
        .subjects
        .iter()
        .any(|subject| subject.quantity_series.is_some())
    {
        return;
    }
    if serde_json::to_writer(Budget(OUTPUT_CAPACITY), &view).is_err() {
        for subject in &mut view.subjects {
            subject.quantity_series = None;
        }
    }
}

#[cfg(test)]
#[path = "../../../../../semantics/face/tests/common/quantity_series.rs"]
mod fixture;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acknowledged_checked_mask_keeps_shared_series_and_complete_reading() {
        let mut face = fixture::fixture();
        let body = conduit_body::Body::born(
            "source/series-owner".into(),
            "checked/series-owner".into(),
            1,
            "sign/series-owner".into(),
        )
        .unwrap();
        face.basis.body_id = Some(body.body_id.clone());
        face.reseal_prepared().unwrap();
        let original = face.clone();
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                visual_constraints: None,
                body_id: body.body_id,
                host_id: "host/series-browser".into(),
                boot_id: "boot/series-browser".into(),
            },
            face,
            1,
            false,
        )
        .unwrap();
        let prepared = mask.view();
        mask.acknowledge(Acknowledgement {
            show_id: prepared.show_id,
            face_id: prepared.face_id,
            face_revision: prepared.face_revision,
        })
        .unwrap();
        let view = serde_json::to_value(mask.view()).unwrap();
        assert_eq!(view["show_state"], "available");
        let subjects = view["subjects"].as_array().unwrap();
        assert_eq!(subjects.len(), original.subjects.len());
        let channel = subjects
            .iter()
            .find(|subject| subject["identity"] == "voltage")
            .unwrap();
        assert_eq!(
            channel["quantity_series"]["plot"]["points"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(channel["quantity_series"]["plot"]["unavailable_samples"], 1);
        assert_eq!(mask.face, original);
        // Exercise the fixed ABI budget with a complete reading near its limit.
        // Optional display data must be removable without losing any reading byte.
        let mut dense = mask.view();
        for subject in &mut dense.subjects {
            subject.quantity_series = None;
        }
        let baseline = serde_json::to_vec(&dense).unwrap().len();
        dense.subjects[0]
            .text
            .push("x".repeat(OUTPUT_CAPACITY - baseline - 8));
        let complete = serde_json::to_vec(&dense).unwrap();
        assert!(complete.len() < OUTPUT_CAPACITY);
        let mut accepted = 0;
        dense.subjects[1].quantity_series =
            prepare(&mask.face, &mask.face.subjects[1], &mut accepted);
        assert!(serde_json::to_vec(&dense).unwrap().len() > OUTPUT_CAPACITY);
        fit_view(&mut dense);
        assert_eq!(serde_json::to_vec(&dense).unwrap(), complete);
        assert_eq!(mask.face, original);
        if let Some(path) = std::env::var_os("CONDUIT_HISTORY_OWNER_VIEW") {
            std::fs::write(path, serde_json::to_vec(&view).unwrap()).unwrap();
        }
    }
    #[test]
    fn interpretation_refusal_and_presentation_capacity_keep_the_source_face() {
        let mut face = fixture::fixture();
        let original = face.clone();
        let subject = &face.subjects[1];
        let mut accepted = 8;
        let view = prepare(&face, subject, &mut accepted).unwrap();
        assert_eq!(view.refusal, Some("quantity-series-capacity"));
        assert!(view.plot.is_none());
        assert_eq!(face, original);
        face.properties
            .iter_mut()
            .find(|property| property.name == "clock")
            .unwrap()
            .value = conduit_face::FacePropertyValue::Text("unknown-clock".into());
        face.reseal_prepared().unwrap();
        let original = face.clone();
        let mut accepted = 0;
        let view = prepare(&face, &face.subjects[1], &mut accepted).unwrap();
        assert_eq!(view.refusal, Some("quantity-series-clock"));
        assert!(view.plot.is_none());
        assert_eq!(accepted, 0);
        assert_eq!(face, original);
    }
}
