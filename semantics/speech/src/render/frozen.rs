use super::*;

/// Frozen tapes copied during admitted preparation. Traversal stores only its
/// cursor; callers stage a copy and commit it with the kernel output action.
#[cfg(all(feature = "semantic-bindings", feature = "kernel"))]
pub(crate) struct FrozenRender {
    events: alloc::vec::Vec<VoiceEvent>,
    frames: alloc::vec::Vec<i32>,
    controls: alloc::vec::Vec<SpeechEventVoiceControl>,
    initial: RenderCursor,
}
#[cfg(all(feature = "semantic-bindings", feature = "kernel"))]
impl FrozenRender {
    pub(crate) fn capture(renderer: Renderer<'_>) -> Result<Self, RenderRefusal> {
        if renderer.rendered_frames() != 0 || renderer.pitch.is_some() {
            return Err(RenderRefusal::ControlAfterStart);
        }
        Ok(Self {
            events: renderer.events.to_vec(),
            frames: renderer
                .frame_counts
                .ok_or(RenderRefusal::TimingCount)?
                .to_vec(),
            controls: renderer
                .controls
                .ok_or(RenderRefusal::ControlCount)?
                .to_vec(),
            initial: renderer.cursor,
        })
    }
    pub(crate) fn initial(&self) -> RenderCursor {
        self.initial
    }
    pub(crate) fn render(
        &self,
        cursor: &mut RenderCursor,
        output: &mut [i16],
    ) -> Result<usize, RenderRefusal> {
        cursor.render_prepared(
            &self.events,
            Some(&self.frames),
            Some(&self.controls),
            None,
            output,
        )
    }
}
