//! Finite attached raster carrier. The provider owns drawing and commits a bounded
//! pixel frame before this module completes either renderer or supplement call.
use super::*;
use conduit_core::*;
use sha2::{Digest, Sha256};

const PIXEL_BOUND: usize = 16 * 1024 * 1024;

/// No deserialization or public receipt constructor: only a successful attached
/// provider callback can create this evidence. It is never a terminal receipt.
struct VisualEffectReceipt {
    show: MaskShow,
    bytes: usize,
    digest: [u8; 32],
}
impl VisualEffectReceipt {
    fn paint<F>(
        show: &MaskShow,
        face: &Face,
        wording: Option<&str>,
        paint: &mut F,
    ) -> Result<Self, TerminalError>
    where
        F: FnMut(&Face, Option<&str>) -> Result<Vec<u8>, TerminalError>,
    {
        let pixels = paint(face, wording)?;
        if pixels.is_empty() || pixels.len() > PIXEL_BOUND {
            return Err(error(
                "raster provider returned no frame or exceeded its finite pixel bound",
            ));
        }
        Ok(Self {
            show: show.clone(),
            bytes: pixels.len(),
            digest: Sha256::digest(&pixels).into(),
        })
    }
}

pub(crate) struct HostedVisualMaskExecution {
    execution: HostedTerminalMaskExecution,
    last_frame_digest: Option<[u8; 32]>,
}
impl HostedVisualMaskExecution {
    pub(crate) fn attached_capabilities() -> Result<Vec<CapabilityOffer>, String> {
        let mut offers = planning::attached_terminal_capabilities();
        for offer in &mut offers {
            let suffix = match offer.capability_id.as_str() {
                "terminal/tee" => "tee",
                "terminal/render" => "render",
                "terminal/input" => "input",
                _ => return Err("unexpected carrier capability".into()),
            };
            offer.capability_id = format!("visual/{suffix}").into();
            offer.implementation.execution_profile_id = "std/raster-mask@1".into();
            offer.implementation.artifact_id = "std/raster-mask@1".into();
            if suffix == "render" {
                offer.implementation.implementation_id = "face/renderer-raster@1".into();
                offer.host_calls[0].target_kind = Some(kind_id(VISUAL_TARGET));
            } else if suffix == "input" {
                offer.implementation.implementation_id = "face/raster-input@1".into();
            }
            for resource in &mut offer.resource_requirements {
                resource.class_id = match resource.class_id.as_str() {
                    "conduit.resource/terminal-output@1" => {
                        "conduit.resource/raster-output@1".into()
                    }
                    "conduit.resource/terminal-input@1" => "conduit.resource/raster-input@1".into(),
                    _ => resource.class_id.clone(),
                };
            }
        }
        offers.push(crate::flow_activation::parsec_instruction_source_offer::offer()?);
        let mut effect = parsec_instruction_effect::offer()?;
        effect.capability_id = VISUAL_EFFECT.into();
        effect.implementation.execution_profile_id = "std/raster-mask@1".into();
        effect.implementation.artifact_id = "std/raster-mask@1".into();
        effect.implementation.implementation_id = VISUAL_EFFECT.into();
        effect.resource_requirements[0].class_id =
            "conduit.resource/raster-instruction-operation@1".into();
        offers.push(effect);
        offers.push(crate::flow_activation::required_parsec_input_instruction_offer()?);
        Ok(offers)
    }
    pub(crate) fn attached_resources() -> Vec<ResourceOffer> {
        vec![
            resource_offer("visual/input", "conduit.resource/raster-input@1", 1),
            resource_offer("visual/output", "conduit.resource/raster-output@1", 1),
            resource_offer(
                "visual/instruction-operation",
                "conduit.resource/raster-instruction-operation@1",
                1,
            ),
        ]
    }
    pub(crate) fn new_attached(host: &HostAdvertisement) -> Result<Self, TerminalError> {
        let required = Self::attached_capabilities().map_err(error)?;
        if required
            .iter()
            .any(|offer| !host.capabilities.contains(offer))
            || Self::attached_resources()
                .iter()
                .any(|resource| !host.resources.contains(resource))
        {
            return Err(error(
                "current Host lacks exact attached raster provider offers",
            ));
        }
        // Select this actual provider, rather than competing terminal/browser
        // renderer offers that may also exist on the same Host.
        let mut selected = host.clone();
        selected.capabilities.retain(|offer| {
            !matches!(
                offer.kind_id.as_str(),
                conduit_face::RENDERER_KIND
                    | conduit_face::FACE_INTERACTION_KIND
                    | conduit_face::FACE_TEE_KIND
                    | parsec_instruction_effect::KIND
            ) || required.contains(offer)
        });
        let source = include_str!("parsec_terminal.conduit")
            .replace("parsec/terminal-mask", "parsec/raster-mask");
        let planned =
            planning::plan_on_host_source(host, &selected, &source, "parsec/raster-mask")?;
        let mut execution = HostedTerminalMaskExecution::with_plan(host, planned, false)?;
        execution.visual = true;
        Ok(Self {
            execution,
            last_frame_digest: None,
        })
    }
    pub(crate) fn current_show(&self) -> Option<&MaskShow> {
        self.execution
            .pending
            .as_ref()
            .filter(|play| play.awaiting_input && !play.retired)
            .map(|play| &play.show)
    }
    pub(crate) fn planned_mask(&self) -> &PlannedMaskPlot {
        self.execution.planned_mask()
    }
    pub(crate) fn bind_parsec_body_scope(
        &mut self,
        plan: &conduit_body::BodyPlan,
        play: &conduit_body::BodyPlayIdentity,
        maximum_shows: u16,
    ) -> Result<(), TerminalError> {
        self.execution
            .bind_parsec_body_scope(plan, play, maximum_shows)
    }
    pub(crate) fn present<F>(
        &mut self,
        face: &Face,
        mut paint: F,
    ) -> Result<MaskShow, TerminalError>
    where
        F: FnMut(&Face, Option<&str>) -> Result<Vec<u8>, TerminalError>,
    {
        let result = (|| {
            let show = self.execution.begin_render(face)?;
            let receipt = VisualEffectReceipt::paint(&show, face, None, &mut paint)?;
            let available =
                self.execution
                    .complete_carrier_render(&receipt.show, receipt.bytes, 1)?;
            self.last_frame_digest = Some(receipt.digest);
            Ok(available)
        })();
        if result.is_err() {
            let _ = self.execution.cancel();
        }
        result
    }
    pub(crate) fn execute_instruction<F>(
        &mut self,
        qualified: crate::installed_std::body_kernel::QualifiedParsecInstruction,
        host: &HostAdvertisement,
        mut paint: F,
    ) -> Result<crate::body_execution::ParsecInstructionEffectObservation, TerminalError>
    where
        F: FnMut(&Face, Option<&str>) -> Result<Vec<u8>, TerminalError>,
    {
        let show = self
            .execution
            .pending
            .as_ref()
            .ok_or_else(|| error("raster instruction has no current Show"))?
            .show
            .clone();
        let mut digest = None;
        let result = self
            .execution
            .execute_instruction_effect(qualified, host, |face, wording| {
                let receipt = VisualEffectReceipt::paint(&show, face, Some(wording), &mut paint)?;
                digest = Some(receipt.digest);
                Ok(())
            });
        if result.is_err() {
            let _ = self.execution.cancel();
        } else {
            self.last_frame_digest = digest;
        }
        result
    }
    pub(crate) fn frame_digest(&self) -> Option<[u8; 32]> {
        self.last_frame_digest
    }
    pub(crate) fn interact(
        &mut self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, TerminalError> {
        self.execution.interact(interaction)
    }
    pub(crate) fn cancel(&mut self) -> Result<(), TerminalError> {
        self.last_frame_digest = None;
        self.execution.cancel()
    }
}
