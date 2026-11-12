//! Existing native semantic routing over a persistent acknowledged Show. No
//! action IDs, route authority or physical readiness are inferred from pixels.
use super::*;
use conduitos::native_face_scene::{FaceFrame, FaceSceneError, FaceSceneInput, NativeFaceScene};

pub(super) enum PreparedSelectedScene {
    ReadOnly(PreparedStaticNativeFaceScene),
    Interactive(NativeFaceScene),
}
impl PreparedSelectedScene {
    pub(super) fn prepare(
        face: Face,
        width: u16,
        height: u16,
        interactive: bool,
    ) -> Result<Self, FaceSceneError> {
        if interactive {
            NativeFaceScene::prepare(face, width, height).map(Self::Interactive)
        } else {
            PreparedStaticNativeFaceScene::prepare(face, width, height).map(Self::ReadOnly)
        }
    }
    pub(super) fn frame(&self) -> Result<FaceFrame<'_>, FaceSceneError> {
        match self {
            Self::ReadOnly(scene) => scene.frame(),
            Self::Interactive(scene) => scene.frame(),
        }
    }
    pub(super) fn retained_backing_bytes(&self) -> Option<usize> {
        match self {
            Self::ReadOnly(scene) => scene.retained_backing_bytes(),
            Self::Interactive(scene) => scene.standard_interactive_retained_backing_bytes(),
        }
    }
    pub(super) fn into_interactive(self) -> Option<NativeFaceScene> {
        match self {
            Self::Interactive(scene) => Some(scene),
            Self::ReadOnly(_) => None,
        }
    }
}
/// Borrowed access keeps the exact correlation, acknowledged Show, completed
/// kernel evidence and original frame charge together until explicit retirement.
pub struct SelectedNativeInteraction {
    pub(in crate::selected_native_mask) correlation: conduit_face::MaskInteractionCorrelation,
    pub(in crate::selected_native_mask) show: conduit_face::MaskShow,
    pub(in crate::selected_native_mask) evidence: Option<SelectedNativeMaskEvidence>,
}
impl SelectedNativeInteraction {
    pub fn correlation(&self) -> &conduit_face::MaskInteractionCorrelation {
        &self.correlation
    }
    pub fn show(&self) -> &conduit_face::MaskShow {
        &self.show
    }
}
pub enum SelectedNativeMaskInput {
    Unchanged,
    Redrawn,
    Submitted(SelectedNativeInteraction),
}
impl BoundReadOnlyMask {
    pub(in crate::selected_native_mask) fn input_geometry(&self) -> (u16, u16) {
        (self.width, self.height)
    }
    pub(in crate::selected_native_mask) fn input_face(&self) -> Option<&Face> {
        self.session.as_ref()?;
        self.scene.as_ref().map(|scene| scene.face())
    }
    pub(super) fn interactive(&self) -> bool {
        self.plan
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .any(|gear| {
                gear.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND
                    && gear.execution_profile_id.as_str() == "conduitos/bounded-interaction@1"
            })
    }
    pub(in crate::selected_native_mask) fn pointer(
        &mut self,
        x: u32,
        y: u32,
        sequence: u64,
        activation: bool,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<SelectedNativeMaskInput, String> {
        if self.failed || !self.interactive() {
            return Err("selected native interactive session unavailable".into());
        }
        let Some(session) = self.session.as_ref() else {
            return Ok(SelectedNativeMaskInput::Unchanged);
        };
        let show = session.show().clone();
        if activation {
            let result = conduitos::native_face_mask::route_acknowledged_native_pointer(
                &mut self.compositor,
                self.scene.as_mut().ok_or("native input scene absent")?,
                &show,
                x,
                y,
                sequence,
            )
            .map_err(|error| format!("native semantic input:{error:?}"))?;
            return self.accept_input(result, show, output);
        } else {
            conduitos::native_face_mask::route_acknowledged_native_pointer_motion(
                &mut self.compositor,
                &show,
                x,
                y,
            )
            .map_err(|error| format!("native pointer route:{error:?}"))?;
        }
        self.commit_input(output)?;
        Ok(SelectedNativeMaskInput::Redrawn)
    }
    pub(in crate::selected_native_mask) fn key(
        &mut self,
        event: conduit_human::KeyEvent,
        sequence: u64,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<SelectedNativeMaskInput, String> {
        if self.failed || !self.interactive() {
            return Err("selected native interactive session unavailable".into());
        }
        let Some(session) = self.session.as_ref() else {
            return Ok(SelectedNativeMaskInput::Unchanged);
        };
        let show = session.show().clone();
        let result = conduitos::native_face_mask::route_acknowledged_native_key(
            &self.compositor,
            self.scene.as_mut().ok_or("native input scene absent")?,
            &show,
            event,
            sequence,
        )
        .map_err(|error| format!("native keyboard route:{error:?}"))?;
        self.accept_input(result, show, output)
    }
    fn accept_input(
        &mut self,
        result: FaceSceneInput,
        show: conduit_face::MaskShow,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<SelectedNativeMaskInput, String> {
        match result {
            FaceSceneInput::Submitted { interaction, .. } => {
                self.session
                    .as_ref()
                    .ok_or("native session absent")?
                    .validate_interaction(&interaction)
                    .map_err(|error| format!("native interaction preflight:{error:?}"))?;
                let (correlation, storage, evidence) = self
                    .session
                    .take()
                    .ok_or("native session absent")?
                    .interact_retained(*interaction)
                    .map_err(|error| {
                        self.failed = true;
                        format!("native input kernel completion:{error:?}")
                    })?;
                let evidence = self.complete_frame(storage, evidence)?;
                return Ok(SelectedNativeMaskInput::Submitted(
                    SelectedNativeInteraction {
                        correlation,
                        show,
                        evidence: Some(evidence),
                    },
                ));
            }
            FaceSceneInput::Changed => {
                self.repaint_input(&show, output)?;
                return Ok(SelectedNativeMaskInput::Redrawn);
            }
            FaceSceneInput::Unchanged => return Ok(SelectedNativeMaskInput::Unchanged),
        }
    }
    fn commit_input(
        &mut self,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<(), String> {
        let result = self
            .compositor
            .compose_frame(output)
            .map(|_| ())
            .map_err(|error| format!("native input compose:{error:?}"))
            .and_then(|()| {
                output
                    .commit_frame()
                    .map_err(|error| format!("native input commit:{error:?}"))
            });
        if result.is_err() {
            // Failed pixels revoke this Show; retain uncertain backing custody.
            self.failed = true;
            self.session.take();
            let _ = self.compositor.remove_surface(SURFACE);
        }
        result
    }
    fn repaint_input(
        &mut self,
        show: &conduit_face::MaskShow,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<(), String> {
        let frame = self
            .scene
            .as_ref()
            .ok_or("native input scene absent")?
            .frame()
            .map_err(|error| format!("native local frame:{error:?}"))?;
        let receipt = self
            .compositor
            .repaint_mask_surface(
                self.scene
                    .as_ref()
                    .ok_or("native input scene absent")?
                    .face(),
                show,
                &self.base,
                &frame.scene,
            )
            .map_err(|error| format!("native local raster:{error:?}"))?
            .clone();
        self.commit_input(output)?;
        if self
            .compositor
            .scanout_acknowledgement(&receipt, show.face_revision)
            .is_none()
        {
            self.failed = true;
            return Err("native local repaint lacks scanout acknowledgement".into());
        }
        Ok(())
    }
}
