//! Trusted local zero-Body encounter over the same Mask execution path.
//! No membership route is fabricated; a local owner applies delivered proposals.
use super::*;

pub(crate) struct LocalBrowserMask(OwnerBrowserMask);

impl LocalBrowserMask {
    pub(crate) fn prepare(
        host_id: HostId,
        boot_id: BootId,
        face: Face,
        source: &str,
        root: &str,
        sequence: u64,
    ) -> Result<Self, String> {
        face.validate()
            .map_err(|error| format!("local Face: {error:?}"))?;
        if face.basis.body_id.is_some() {
            return Err("local Mask requires a zero-Body Face".into());
        }
        let host =
            crate::installed_browser::membership_advertisement(host_id.clone(), boot_id.clone());
        let planned = crate::installed_browser::dom_mask::planned_local_mask(&host, source, root)?;
        OwnerBrowserMask::prepare_context(
            preparation::MaskHostBasis {
                body_id: None,
                host_id,
                boot_id,
                visual_constraints: None,
            },
            face,
            planned,
            None,
            None,
            sequence,
            true,
        )
        .map(Self)
    }

    pub(crate) fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"face": self.0.face, "mask_show": self.0.show,
            "planned_mask": self.0.planned, "mask_play": self.0.play,
            "view": self.0.view()})
    }

    pub(crate) fn acknowledge(
        &mut self,
        show: &str,
        face: &str,
        revision: &str,
    ) -> Result<(), String> {
        self.0.acknowledge(Acknowledgement {
            show_id: show.into(),
            face_id: face.into(),
            face_revision: revision.into(),
        })
    }

    pub(crate) fn interact(
        &mut self,
        proposal: serde_json::Value,
    ) -> Result<(MaskShow, FaceInteraction), String> {
        let proposed: LocalInteraction = serde_json::from_value(proposal)
            .map_err(|error| format!("local interaction: {error}"))?;
        if !self.0.interactions_admitted
            || proposed.show_id != self.0.show.show_id.as_str()
            || proposed.face_id != self.0.face.identity.as_str()
            || proposed.face_revision != self.0.face.revision.to_string()
        {
            return Err("local interaction has a stale or unadmitted Show".into());
        }
        let interaction = FaceInteraction::new(
            &self.0.face,
            &self.0.show,
            &proposed.action_id,
            &proposed.target,
            proposed.arguments,
            proposed.sequence,
        )
        .map_err(|error| format!("local typed interaction: {error:?}"))?;
        let emission = self.0.emit_interaction(interaction)?;
        Ok((emission.show.clone(), emission.interaction))
    }

    pub(crate) fn cancel(&mut self) -> Result<(), String> {
        self.retire(ManifestationLifecycle::Closed)
    }

    pub(crate) fn replace(&mut self) -> Result<(), String> {
        self.retire(ManifestationLifecycle::Replaced)
    }

    fn retire(&mut self, lifecycle: ManifestationLifecycle) -> Result<(), String> {
        self.0
            .scheduler
            .cancel()
            .map_err(|error| format!("cancel local Mask: {error:?}"))?;
        self.0.interactions_admitted = false;
        self.0.show = self
            .0
            .show
            .transition(
                lifecycle,
                SignId::from(format!(
                    "sign/browser-local-retired/{}",
                    self.0.face.revision
                )),
            )
            .map_err(|error| format!("close local Show: {error:?}"))?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalInteraction {
    show_id: String,
    face_id: String,
    face_revision: String,
    action_id: String,
    target: String,
    arguments: Vec<FaceInteractionArgument>,
    sequence: u64,
}
