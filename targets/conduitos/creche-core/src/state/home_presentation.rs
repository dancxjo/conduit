//! Presentation changes only bounded outgoing bytes, never the Home controller.
use super::*;
use crate::home::presentation::{Content, Request as Presentation, digest};
impl State {
    pub(super) fn home_scene(&mut self, bytes: &[u8]) -> Result<Response, Refusal> {
        let input: Presentation = data::decode(bytes)?;
        input.validate()?;
        let scene = self.home_current(input.scope.revision)?.scene(&input)?;
        let length = scene.encoded_len();
        let encoded = scene.encode();
        let mut output = Vec::new();
        output
            .try_reserve_exact(32 + length)
            .map_err(|_| Refusal::Capacity)?;
        output.extend_from_slice(&digest(bytes));
        output.extend_from_slice(&encoded[..length]);
        self.outgoing = output;
        self.read_offset = 0;
        Ok(Response::Blob {
            kind: OutputKind::HomeScene,
            revision: input.scope.revision,
            bytes: self.outgoing.len() as u32,
        })
    }
    pub(super) fn home_content(&mut self, bytes: &[u8]) -> Result<Response, Refusal> {
        let input: Presentation = data::decode(bytes)?;
        input.validate()?;
        let (properties, text) = self
            .home_current(input.scope.revision)?
            .face_content(&input);
        let content = Content {
            scope: input.scope,
            request_digest: digest(bytes),
            properties,
            text,
        };
        self.home_publish(OutputKind::HomeContent, &content)
    }
}
