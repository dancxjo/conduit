//! Projection of Conduit's lifecycle truth into the shell status Presentation.

use alloc::vec;

use conduit_presentation::{
    Presentation, PresentationBasis, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject,
};

pub(super) fn with_presenter_host(
    mut presentation: Presentation,
    host: &conduit_core::HostId,
) -> Result<Presentation, TourShellError> {
    let item = presentation
        .text
        .iter_mut()
        .find(|item| item.subject == "tour/status/host")
        .ok_or(TourShellError::Identity)?;
    let identity = host.as_str();
    // Eleven ASCII cells also fit the compact status item at 640 pixels.
    let short: alloc::string::String = identity.chars().take(8).collect();
    item.text = alloc::format!(
        "Native\n{short}{}",
        if short.len() < identity.len() {
            "..."
        } else {
            ""
        }
    );
    presentation.subjects.push(PresentationSubject {
        identity: identity.into(),
        role: PresentationRole::Host,
        label: "Native presenter Host".into(),
        accessibility_name: alloc::format!("Native presenter Host {identity}"),
    });
    presentation.relationships.push(PresentationRelationship {
        source: "tour/status/host".into(),
        target: identity.into(),
        kind: PresentationRelationshipKind::Observes,
    });
    Presentation::new(
        presentation.revision,
        presentation.basis,
        presentation.subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
    )
    .map_err(|_| TourShellError::Identity)
}

use crate::{display::PixelTarget, product_journey::JourneyProjection, tour_product::TourProduct};

use super::{ShellPresentationReceipt, Slot, TourShellError, TourShellPresenter};

/// Retained presentation input, copied only from the authoritative journey.
pub(super) struct StatusSnapshot {
    name: Option<alloc::string::String>,
    status: crate::product_journey::JourneyStatus,
}

pub(super) fn with_status(
    mut presentation: Presentation,
    snapshot: Option<&StatusSnapshot>,
) -> Result<Presentation, TourShellError> {
    use crate::product_journey::JourneyStatus;
    let Some(snapshot) = snapshot else {
        return Ok(presentation);
    };
    let basis = &presentation.basis;
    let body = if basis.body_id.is_some() {
        snapshot.name.as_deref().unwrap_or("Retained")
    } else {
        "Absent"
    };
    let wake = if basis.wake_id.is_none() {
        "Absent"
    } else {
        match snapshot.status {
            JourneyStatus::BornLulled | JourneyStatus::Lulled => "Lulled",
            _ => "Awake",
        }
    };
    let plan = if basis.plan_id.is_none() {
        "Absent"
    } else {
        match snapshot.status {
            JourneyStatus::Planned => "Ready",
            JourneyStatus::Playing => "In use",
            _ => "Retained",
        }
    };
    let play = if basis.active_play_id.is_none() {
        "Inactive"
    } else {
        match snapshot.status {
            JourneyStatus::Playing => "Running",
            JourneyStatus::ResultVisible => "Completed",
            JourneyStatus::Stopped => "Stopped",
            JourneyStatus::Lulled => "Ended",
            _ => "Recorded",
        }
    };
    for (key, value) in [
        ("body", body),
        ("wake", wake),
        ("plan", plan),
        ("play", play),
    ] {
        let identity = alloc::format!("tour/status/{key}");
        let item = presentation
            .text
            .iter_mut()
            .find(|item| item.subject == identity)
            .ok_or(TourShellError::Identity)?;
        item.text = value.into();
    }
    Presentation::new(
        presentation.revision,
        presentation.basis,
        presentation.subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
    )
    .map_err(|_| TourShellError::Identity)
}

pub(super) fn empty_lifecycle_basis() -> PresentationBasis {
    PresentationBasis {
        body_id: None,
        wake_id: None,
        source_document_id: None,
        checked_form_id: None,
        expanded_form_id: None,
        plan_id: None,
        active_play_id: None,
        sign_ids: vec![],
    }
}

pub(super) fn basis_from_projection(lifecycle: &JourneyProjection) -> PresentationBasis {
    let embodied = lifecycle.body_id.is_some();
    let planned = lifecycle.plan_id.is_some();
    let mut sign_ids = lifecycle
        .born_sign_id
        .iter()
        .chain(lifecycle.input_sign_id.iter())
        .chain(lifecycle.result_sign_id.iter())
        .cloned()
        .collect::<alloc::vec::Vec<_>>();
    sign_ids.sort();
    sign_ids.dedup();
    PresentationBasis {
        body_id: lifecycle.body_id.clone(),
        wake_id: embodied.then(|| lifecycle.wake_id.clone()).flatten(),
        source_document_id: Some(lifecycle.source_document_id.clone()),
        checked_form_id: Some(lifecycle.checked_form_id.clone()),
        expanded_form_id: embodied.then(|| lifecycle.expanded_form_id.clone()),
        plan_id: embodied.then(|| lifecycle.plan_id.clone()).flatten(),
        active_play_id: planned.then(|| lifecycle.active_play_id.clone()).flatten(),
        sign_ids,
    }
}

impl TourShellPresenter {
    pub fn present_with_lifecycle(
        &mut self,
        tour: &TourProduct,
        lifecycle: &JourneyProjection,
        display: &mut impl PixelTarget,
    ) -> Result<ShellPresentationReceipt, TourShellError> {
        self.lifecycle_revision = lifecycle.revision;
        self.lifecycle_basis = basis_from_projection(lifecycle);
        self.lifecycle_status = Some(StatusSnapshot {
            name: lifecycle.friendly_name.clone(),
            status: lifecycle.status,
        });
        self.present(tour, display)
    }

    pub fn has_transient(&self) -> bool {
        self.surfaces
            .iter()
            .any(|surface| surface.slot == Slot::Transient && surface.admitted)
    }

    /// Release retained shell storage before the WORLD front door resumes the
    /// finite native display service.
    pub fn suspend(&mut self) -> Result<(), TourShellError> {
        for slot in Slot::ALL {
            self.dismiss(slot)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_host_projection_keeps_full_identity_without_inventing_lines_or_membership() {
        let state = conduit_tour_model::TourWorkspaceState::canonical(
            1,
            conduit_tour_model::TourWorkspacePhase::LessonReady,
        );
        let original = state.status_presentation().unwrap();
        let host = conduit_core::HostId::from("0123456789abcdef0123456789abcdef");
        let presentation = with_presenter_host(original.clone(), &host).unwrap();
        assert_eq!(presentation.basis, original.basis);
        assert!(presentation.basis.body_id.is_none());
        assert!(
            presentation
                .subjects
                .iter()
                .any(|subject| subject.role == PresentationRole::Host
                    && subject.identity == host.as_str())
        );
        assert!(
            presentation
                .relationships
                .iter()
                .any(|edge| edge.source == "tour/status/host"
                    && edge.target == host.as_str()
                    && edge.kind == PresentationRelationshipKind::Observes)
        );
        assert!(
            presentation.text.iter().any(
                |item| item.subject == "tour/status/host" && item.text == "Native\n01234567..."
            )
        );
        assert_eq!(
            crate::display::text_height("Host\nNative\n01234567...", 640 / 6 - 16).unwrap(),
            48
        );
        #[cfg(feature = "native-compositor")]
        {
            let height = crate::display::typography::TextLayout::new(
                "Host\nNative\n01234567...",
                crate::display::typography::TextRole::Label,
                640 / 6 - 16,
            )
            .unwrap()
            .finish_height();
            assert!(height <= u32::from(crate::tour_workspace::STATUS_HEIGHT - 12));
        }
        assert!(
            presentation
                .text
                .iter()
                .any(|item| item.subject == "tour/status/lines" && item.text == "Unobserved")
        );
    }
}
