//! Read-only guest projection requests; all returned content is checked in Root.
use super::*;
use conduit_face::{GraphicsScene, LayoutRect};
use conduitos_creche_core::data::{
    NativeFaceOutput, NativeSceneRequest, NativeScope, scene_request_digest,
};
impl CrecheDraft {
    pub(crate) fn native_scene(
        &self,
        scope: NativeScope,
        width: u16,
        height: u16,
    ) -> Result<GraphicsScene, TransportError> {
        let mut service = self
            .service
            .try_borrow_mut()
            .map_err(|_| TransportError::Terminated)?;
        let result = (|| {
            if scope.revision != self.revision() {
                return Err(TransportError::Protocol);
            }
            scope.validate().map_err(|_| TransportError::Bounds)?;
            let input = encode(&NativeSceneRequest {
                scope,
                width,
                height,
            })?;
            let response = service.input(InputKind::NativeScene, &input)?;
            let output = service.blob(response, OutputKind::NativeScene, self.revision())?;
            if output.get(..32) != Some(scene_request_digest(&input).as_slice()) {
                return Err(TransportError::Protocol);
            }
            let scene =
                GraphicsScene::decode(&output[32..]).map_err(|_| TransportError::Protocol)?;
            let screen = LayoutRect {
                x: 0,
                y: 0,
                width,
                height,
            };
            if scene
                .commands()
                .iter()
                .any(|command| command.clip != screen)
            {
                return Err(TransportError::Protocol);
            }
            Ok(scene)
        })();
        if result.is_err() {
            service.quarantine();
        }
        result
    }
    pub(crate) fn native_face(&self, scope: NativeScope) -> Result<Face, TransportError> {
        let mut service = self
            .service
            .try_borrow_mut()
            .map_err(|_| TransportError::Terminated)?;
        let result = (|| {
            if scope.revision != self.revision() {
                return Err(TransportError::Protocol);
            }
            scope.validate().map_err(|_| TransportError::Bounds)?;
            let input = encode(&scope)?;
            let response = service.input(InputKind::NativeFace, &input)?;
            let output = service.blob(response, OutputKind::NativeFace, self.revision())?;
            let returned: NativeFaceOutput = decode(&mut service, &output)?;
            if returned.scope != scope || returned.face.revision != scope.outer_revision {
                return Err(TransportError::Protocol);
            }
            returned
                .face
                .validate()
                .map_err(|_| TransportError::Protocol)?;
            let host = alloc::format!("host/{}/{}", scope.host_id.as_str(), scope.boot_id.as_str());
            if !returned.face.subjects.iter().any(|subject| {
                subject.identity == host && subject.role == conduit_face::FaceRole::Host
            }) {
                return Err(TransportError::Protocol);
            }
            Ok(returned.face)
        })();
        if result.is_err() {
            service.quarantine();
        }
        result
    }
}
