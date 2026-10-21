//! Every projection is a fresh correlated service response, with no Root renderer fallback.
use super::*;
use conduit_face::{FaceProperty, FaceText, GraphicsScene, LayoutRect};
use conduitos_creche_core::home::presentation::{Content, Request as Presentation, digest};
impl Home {
    pub(crate) fn presentation_revision(&self) -> u32 {
        self.projection.revision
    }
    pub(crate) fn scene(&self, input: &Presentation) -> Result<GraphicsScene, TransportError> {
        let mut service = self
            .service
            .try_borrow_mut()
            .map_err(|_| TransportError::Terminated)?;
        let result = (|| {
            input.validate().map_err(|_| TransportError::Bounds)?;
            if input.scope.revision != self.projection.revision {
                return Err(TransportError::Protocol);
            }
            let extent = input.extent.as_ref().ok_or(TransportError::Bounds)?;
            let bytes = encode(input)?;
            let response = service.input(InputKind::HomeScene, &bytes)?;
            let output = service.blob(response, OutputKind::HomeScene, input.scope.revision)?;
            if output.get(..32) != Some(digest(&bytes).as_slice()) {
                return Err(TransportError::Protocol);
            }
            let scene =
                GraphicsScene::decode(&output[32..]).map_err(|_| TransportError::Protocol)?;
            let clip = LayoutRect {
                x: 0,
                y: 0,
                width: extent.width,
                height: extent.height,
            };
            if scene.commands().iter().any(|command| command.clip != clip) {
                return Err(TransportError::Protocol);
            }
            Ok(scene)
        })();
        if result.is_err() {
            service.quarantine();
        }
        result
    }
    pub(crate) fn face_content(
        &self,
        input: &Presentation,
    ) -> Result<(Vec<FaceProperty>, Vec<FaceText>), TransportError> {
        let mut service = self
            .service
            .try_borrow_mut()
            .map_err(|_| TransportError::Terminated)?;
        let result = (|| {
            input.validate().map_err(|_| TransportError::Bounds)?;
            if input.scope.revision != self.projection.revision || input.extent.is_some() {
                return Err(TransportError::Protocol);
            }
            let bytes = encode(input)?;
            let response = service.input(InputKind::HomeContent, &bytes)?;
            let output = service.blob(response, OutputKind::HomeContent, input.scope.revision)?;
            let content: Content = decode(&mut service, &output)?;
            if content.scope != input.scope
                || content.request_digest != digest(&bytes)
                || content.properties.len() > 64
                || content.text.len() > 32
                || content
                    .properties
                    .iter()
                    .any(|property| property.subject != "conduitos/shell/home")
                || content
                    .text
                    .iter()
                    .any(|text| text.subject != "conduitos/shell/home")
            {
                return Err(TransportError::Protocol);
            }
            // The owning FrontDoor builds its outer Face through the existing
            // checked constructor, validating all property/text values and digest.
            Ok((content.properties, content.text))
        })();
        if result.is_err() {
            service.quarantine();
        }
        result
    }
}
