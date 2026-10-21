//! Bounded native presentation over the existing canonical codecs.
use super::*;
use crate::data::{NativeFaceOutput, NativeSceneRequest, NativeScope};
impl State {
    // The Draft scene has the same fixed scene/encoding buffers as Home.
    // Keep them out of the shared dispatch frame and its 64 KiB entry stack.
    #[inline(never)]
    pub(super) fn native_scene(&mut self, bytes: &[u8]) -> Result<Response, Refusal> {
        let input: NativeSceneRequest = data::decode(bytes)?;
        input.scope.validate()?;
        self.current(input.scope.revision)?;
        let scene = self
            .draft
            .as_ref()
            .ok_or(Refusal::Order)?
            .scene(input.width, input.height)?;
        let length = scene.encoded_len();
        let encoded = scene.encode();
        let mut output = Vec::new();
        output
            .try_reserve_exact(32 + length)
            .map_err(|_| Refusal::Capacity)?;
        output.extend_from_slice(&data::scene_request_digest(bytes));
        output.extend_from_slice(&encoded[..length]);
        self.outgoing = output;
        self.read_offset = 0;
        Ok(Response::Blob {
            kind: OutputKind::NativeScene,
            revision: input.scope.revision,
            bytes: self.outgoing.len() as u32,
        })
    }
    pub(super) fn native_face(&mut self, bytes: &[u8]) -> Result<Response, Refusal> {
        let scope: NativeScope = data::decode(bytes)?;
        scope.validate()?;
        self.current(scope.revision)?;
        let face = self
            .draft
            .as_ref()
            .ok_or(Refusal::Order)?
            .native_face(&scope)?;
        self.publish(OutputKind::NativeFace, &NativeFaceOutput { scope, face })
    }
}
