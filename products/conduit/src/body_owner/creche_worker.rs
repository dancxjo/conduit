//! Scoped Crèche execution on the service's existing Body worker and Host.
use super::{creche_route_custody::CrecheRouteCustody, Owner};
use conduit::creche_encounter::{
    CrecheBodyBinding, CrecheEncounterController, CrecheEncounterFactory,
};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_body_make::deployment_runtime::CrecheOutcome;
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunReport, BodyRunRequest};
use conduit_std_host::{ExternalForeDelivery, StdHost, TimerAdapter};
use std::{io::Write, path::Path, sync::mpsc::SyncSender};

pub(super) struct CrecheLive {
    pub(super) controller: CrecheEncounterController,
    pub(super) custody: CrecheRouteCustody,
}
pub(super) struct CrecheExecution {
    pub(super) factory: CrecheEncounterFactory,
    binding: Option<CrecheBodyBinding>,
    pub(super) custody: CrecheRouteCustody,
}
impl Owner {
    pub(crate) fn install_creche_factory(
        &mut self,
        factory: CrecheEncounterFactory,
    ) -> Result<(), String> {
        if self.creche_factory.is_some()
            || self.host.is_playing()
            || !self
                .host
                .advertisement()
                .resources
                .contains(&factory.resource_offer())
            || !self
                .host
                .advertisement()
                .capabilities
                .contains(&conduit_body_make::deployment_runtime_catalog::creche_std_offer())
        {
            return Err("prepared Crèche factory does not match the idle original Host".into());
        }
        self.creche_factory = Some(factory);
        Ok(())
    }
    pub(super) fn prepare_creche_service_run(
        &mut self,
        root: &Path,
        source: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<Option<CrecheBodyBinding>, String> {
        if source.expanded.name != "creche/main" {
            return Ok(None);
        }
        let resident = self
            .resident
            .as_ref()
            .ok_or("Crèche resident Source absent")?;
        // Exact shipped Source and real installed offer must plan before invitation effects.
        self.shipped_creche_partition(source, resident)?
            .ok_or("Crèche Source absent")?;
        if self.creche_factory.is_none() {
            return Err("Crèche has no retained initialized provider".into());
        }
        let invitation = self.issue_invitation(root, 600, None)?;
        // Private provisioning stays with the owner; never publish this secret in a Face.
        let directory = root.join("body/creche");
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        super::super::super::restrict_directory(&directory)?;
        super::super::super::write_json_atomic(&directory.join("invitation.json"), &invitation)?;
        Ok(Some(CrecheBodyBinding {
            biography: self.session.evidence().clone(),
            invitation: invitation.claim,
        }))
    }
    pub(super) fn take_creche_execution(
        &mut self,
        binding: Option<CrecheBodyBinding>,
    ) -> Option<CrecheExecution> {
        binding.map(|binding| CrecheExecution {
            factory: self
                .creche_factory
                .take()
                .expect("prepared provider retained before Plan"),
            binding: Some(binding),
            custody: CrecheRouteCustody::new(),
        })
    }
    pub(super) fn project_creche_products(
        &self,
        mut face: conduit_face::Face,
    ) -> Result<conduit_face::Face, String> {
        let Some(products) = &self.creche_products else {
            return Ok(face);
        };
        let identity = format!("artifact/{}", products.spore.manifest.spore_id);
        if let Some(parent) = face.subjects.first() {
            face.relationships.push(conduit_face::FaceRelationship {
                source: parent.identity.clone(),
                target: identity.clone(),
                kind: conduit_face::FaceRelationshipKind::Contains,
            });
        }
        face.subjects.push(conduit_face::FaceSubject {
            identity: identity.clone(),
            role: conduit_face::FaceRole::Info,
            name: "Prepared Host artifact".into(),
        });
        for (name, value) in [
            ("spore-id", products.spore.manifest.spore_id.as_str()),
            ("image-id", products.spore.image.manifest.image_id.as_str()),
            ("artifact-disposition", "prepared"),
        ] {
            face.properties.push(conduit_face::FaceProperty {
                subject: identity.clone(),
                name: name.into(),
                value: conduit_face::FacePropertyValue::Identity(value.into()),
            });
        }
        face.text.push(conduit_face::FaceText { subject:identity,text:"Seed and receipt are retained by this owner. Deployment, Boot and joining remain unverified.".into() });
        conduit_face::Face::new_with_semantics(
            face.revision
                .checked_add(1)
                .ok_or("Crèche result revision exhausted")?,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            face.actions,
            face.disclosures,
        )
        .map_err(|e| format!("Crèche retained artifact Face: {e:?}"))
    }
    /// Caller must first validate the actual selected native/browser return route.
    pub(crate) fn submit_validated_creche_return(
        &mut self,
        show: &conduit_face::MaskShow,
        input: &conduit_face::FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let face = self.local_face_snapshot()?;
        let live = self
            .creche_live
            .as_ref()
            .ok_or("Crèche has no active scoped encounter")?;
        live.custody.record_validated_return(&face, show, input)?;
        live.controller.acknowledge_show(show.clone())?;
        live.controller.submit(input.clone())?;
        Ok(
            serde_json::json!({"schema":"conduit.creche/interaction-queued@1","interaction_id":input.identity.as_str(),"face_id":face.identity,"face_revision":face.revision}),
        )
    }
}
struct OutcomeFore(Option<CrecheOutcome>);
impl BodyForeOutputAdapter for OutcomeFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        if self.0.is_some()
            || output.front_port_id != conduit_core::port_id("outcome")
            || output.track != conduit_core::ConnectionTrack::Payload
        {
            return Err("Crèche emitted an unexpected or duplicate outcome Fore".into());
        }
        self.0 = Some(
            CrecheOutcome::decode(&output.bytes).ok_or("Crèche outcome Fore encoding refused")?,
        );
        Ok(())
    }
}
impl CrecheExecution {
    pub(super) fn run<
        W: Write,
        T: TimerAdapter,
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    >(
        &mut self,
        host: &mut StdHost,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        controller: &SyncSender<CrecheEncounterController>,
        started: F,
    ) -> Result<BodyRunReport, String> {
        let binding = self
            .binding
            .take()
            .ok_or("Crèche execution binding already consumed")?;
        let (mut adapter, handle) = self
            .factory
            .encounter(binding, Box::new(self.custody.clone()))?;
        controller
            .send(handle)
            .map_err(|_| "Crèche service controller disconnected")?;
        host.run_body_plan_with_creche_to_with_start(
            request,
            &mut adapter,
            &mut OutcomeFore(None),
            output,
            timer,
            started,
        )
    }
}

#[cfg(test)]
#[path = "creche_worker/tests.rs"]
mod tests;
