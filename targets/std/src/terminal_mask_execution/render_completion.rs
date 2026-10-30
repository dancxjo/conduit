//! Complete the actual renderer Host Call and retain its acknowledged Show.
use super::*;

impl HostedTerminalMaskExecution {
    pub(super) fn complete_carrier_render(
        &mut self,
        prepared_show: &MaskShow,
        bytes_written: usize,
        frame_sequence: u64,
    ) -> Result<MaskShow, TerminalError> {
        let mut play = self
            .pending
            .take()
            .ok_or_else(|| error("no terminal renderer pending"))?;
        if play.awaiting_input
            || prepared_show != &play.show
            || bytes_written == 0
            || frame_sequence == 0
        {
            return Err(TerminalError::StaleShow);
        }
        let available = play
            .show
            .transition(
                ManifestationLifecycle::Available,
                format!(
                    "std/{}/available/{}",
                    if self.visual { "raster" } else { "terminal" },
                    play.show.show.play_sequence
                )
                .into(),
            )
            .map_err(debug_error)?;
        let bytes = serde_json::to_vec(&available).map_err(debug_error)?;
        play.complete_call(Some(&bytes))?;
        play.show = available;
        let mut observed = false;
        let mut pending = None;
        for _ in 0..64 {
            if let Some(request) = play.scheduler.next_host_request() {
                if Some(request.node) == play.instruction_node {
                    play.accept_instruction_request(request, &self.host)?;
                    continue;
                }
                if play.read_only
                    || pending.is_some()
                    || Some(request.node) != play.interaction_node
                    || request.call != HostCallId(0)
                    || request.request != RequestId(1)
                    || play
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(debug_error)?
                        != bytes
                {
                    return Err(error("terminal interaction Host Call mismatch"));
                }
                pending = Some(request);
            }
            if let Some(actual) = play.take_output(&play.show_fore.clone())? {
                if observed || actual != bytes {
                    return Err(error("terminal Show Fore mismatch"));
                }
                let actual_show: MaskShow = serde_json::from_slice(&actual).map_err(debug_error)?;
                actual_show.validate(&play.face).map_err(debug_error)?;
                play.show = actual_show;
                observed = true;
            }
            if observed && play.read_only {
                play.finish(None)?;
                return Ok(play.show.clone());
            }
            if observed && pending.is_some() {
                play.request = pending.take().expect("observed pending Host Call");
                play.awaiting_input = true;
                let show = play.show.clone();
                self.pending = Some(play);
                return Ok(show);
            }
            step(&mut play.scheduler)?;
        }
        Err(error("terminal Show Fore work bound exhausted"))
    }
}
