//! Preserve the explicitly selected presentation through each finite return.
use super::{Grant, MaskSelection};
use conduit_core::HostAdvertisement;
use conduit_face::RemoteOwnerMaskRouteSeal;
use conduit_std_host::secure_websocket::SecureWebSocketListener;
use std::path::Path;

impl Grant {
    pub(crate) fn seal_mask_route(
        &self,
        listener: &SecureWebSocketListener,
        state_dir: &Path,
        owner: &HostAdvertisement,
        sequence: u8,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let binding = super::super::native_lines::binding_reference(
            listener,
            &self.binding_reference(sequence),
        );
        if let Some(selection) = &self.spoken_selection {
            let (face, show, _) =
                super::super::native_lines::offer_terminal(owner, &self.member, &binding);
            return crate::durable_host_control::select_native_mask_route(
                state_dir,
                self.member.clone(),
                face,
                show,
                None,
                Some(selection.clone()),
                self.expires_at_millis,
            );
        }
        let (face, show, interaction) = match self.mask {
            MaskSelection::Native => {
                let (face, show) =
                    super::super::native_lines::offer_pair(owner, &self.member, &binding);
                (face, show, None)
            }
            MaskSelection::Terminal => {
                let (face, show, interaction) =
                    super::super::native_lines::offer_terminal(owner, &self.member, &binding);
                (face, show, Some(interaction))
            }
        };
        crate::durable_host_control::select_native_mask_route(
            state_dir,
            self.member.clone(),
            face,
            show,
            interaction,
            None,
            self.expires_at_millis,
        )
    }
}

impl Grant {
    /// Seal and fetch across separate owner actor requests without publishing a
    /// mixed revision. Only an observed Face transition is retried; failed
    /// authority, pressure, expiry and transport checks are terminal here.
    pub(crate) fn prepare_face_route(
        &self,
        listener: &SecureWebSocketListener,
        state_dir: &Path,
        mut request: conduit_face::OwnerFaceSnapshotRequest,
        sequence: u8,
    ) -> Result<conduit_face::OwnerFaceSnapshotResponse, String> {
        request.last_seen_revision = None;
        request.last_seen_identity = None;
        bounded_prepare(self.expires_at_millis, || {
            let (_, owner) = crate::durable_host_control::local_face_snapshot(state_dir)?;
            let seal = self.seal_mask_route(listener, state_dir, &owner, sequence)?;
            let response = crate::durable_host_control::face_snapshot(state_dir, request.clone())?;
            let conduit_face::OwnerFaceSnapshotResponse::Snapshot { schema, face, .. } = response
            else {
                return Err("full-owner-face-unavailable".into());
            };
            if schema != conduit_face::OWNER_FACE_RESPONSE_SCHEMA {
                return Err("owner-face-response-schema".into());
            }
            if !same_face(&face, &seal.face_id, seal.face_revision, &seal.face_basis) {
                return Ok(None);
            }
            if self.mask != MaskSelection::Terminal
                && !crate::durable_host_control::has_native_return_route_intent(&face)
            {
                return Err("native-return-route-intent-unavailable".into());
            }
            let interactions_admitted = self.spoken_selection.is_none()
                && super::super::face_snapshot::route_allows_interaction(&seal);
            Ok(Some(conduit_face::OwnerFaceSnapshotResponse::Snapshot {
                schema,
                face,
                interactions_admitted,
                route: Some(Box::new(seal)),
            }))
        })
    }
}
fn same_face(
    face: &conduit_face::Face,
    id: &conduit_face::FaceContentId,
    revision: u64,
    basis: &conduit_face::FaceBasis,
) -> bool {
    &face.identity == id && face.revision == revision && &face.basis == basis
}
fn bounded_prepare(
    expires_at_millis: u64,
    mut attempt: impl FnMut() -> Result<Option<conduit_face::OwnerFaceSnapshotResponse>, String>,
) -> Result<conduit_face::OwnerFaceSnapshotResponse, String> {
    for _ in 0..3 {
        if super::current_time_millis()? >= expires_at_millis {
            return Err("return-expired".into());
        }
        let prepared = attempt()?;
        if super::current_time_millis()? >= expires_at_millis {
            return Err("return-expired".into());
        }
        if let Some(response) = prepared {
            return Ok(response);
        }
    }
    Err("owner-face-transition-pressure".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{Face, FaceBasis, FaceRole, FaceSubject, OwnerFaceSnapshotResponse};
    fn face(name: &str) -> Face {
        Face::new(
            1,
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![FaceSubject {
                identity: "front/coherent".into(),
                role: FaceRole::Document,
                name: name.into(),
            }],
            vec![],
            vec![],
            vec![],
        )
        .unwrap()
    }
    #[test]
    fn exact_identity_revision_and_provenance_must_all_agree() {
        let current = face("Current");
        assert!(same_face(
            &current,
            &current.identity,
            current.revision,
            &current.basis
        ));
        assert!(!same_face(
            &current,
            &face("Other").identity,
            current.revision,
            &current.basis
        ));
        assert!(!same_face(
            &current,
            &current.identity,
            current.revision + 1,
            &current.basis
        ));
        let mut changed = current.basis.clone();
        changed.sign_ids.push("sign/other-execution".into());
        assert!(!same_face(
            &current,
            &current.identity,
            current.revision,
            &changed
        ));
    }
    #[test]
    fn only_observed_transition_retries_and_attempts_are_finite() {
        // Retry-control seam only; the sentinel is not an admitted route/Show.
        let mut calls = 0;
        let response = bounded_prepare(u64::MAX, || {
            calls += 1;
            Ok((calls == 3).then(|| OwnerFaceSnapshotResponse::Refused {
                schema: conduit_face::OWNER_FACE_RESPONSE_SCHEMA.into(),
                code: "test-sentinel".into(),
            }))
        })
        .unwrap();
        assert_eq!(calls, 3);
        assert!(matches!(
            response,
            OwnerFaceSnapshotResponse::Refused { .. }
        ));
        calls = 0;
        assert_eq!(
            bounded_prepare(u64::MAX, || {
                calls += 1;
                Ok(None)
            })
            .unwrap_err(),
            "owner-face-transition-pressure"
        );
        assert_eq!(calls, 3);
    }
    #[test]
    fn authority_pressure_and_expiration_do_not_retry() {
        for error in [
            "authority-refused",
            "face-frame-pressure",
            "control-outcome-unknown",
        ] {
            let mut calls = 0;
            assert_eq!(
                bounded_prepare(u64::MAX, || {
                    calls += 1;
                    Err(error.into())
                })
                .unwrap_err(),
                error
            );
            assert_eq!(calls, 1);
        }
        let mut calls = 0;
        assert_eq!(
            bounded_prepare(0, || {
                calls += 1;
                Ok(None)
            })
            .unwrap_err(),
            "return-expired"
        );
        assert_eq!(calls, 0);
    }
}
