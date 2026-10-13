//! Exact Source -> internal Fore -> supplementary write/flush -> effect ACK.
//! The original Show does not acknowledge this later effect.
use super::*;
use crate::installed_std::body_kernel::QualifiedParsecInstruction;
use std::io::Write;

impl HostedTerminalMaskExecution {
    pub(crate) fn execute_instruction<W: Write + ?Sized>(
        &mut self,
        qualified: QualifiedParsecInstruction,
        host: &HostAdvertisement,
        output: &mut W,
        expected: &str,
    ) -> Result<(), TerminalError> {
        let result = self.execute_instruction_inner(qualified, host, output, expected);
        if result.is_err() {
            let _ = self.cancel();
        }
        result
    }
    fn execute_instruction_inner<W: Write + ?Sized>(
        &mut self,
        qualified: QualifiedParsecInstruction,
        host: &HostAdvertisement,
        output: &mut W,
        expected: &str,
    ) -> Result<(), TerminalError> {
        let source_request = self.pending_instruction_request(host)?;
        let (parent_plan, parent) = self
            .body_scope
            .as_ref()
            .ok_or_else(|| error("instruction has no parent custody"))?;
        let play = self
            .pending
            .as_mut()
            .ok_or_else(|| error("instruction has no live Mask Play"))?;
        let active = conduit_core::bind_active_play(
            &self.planned.plan.plan_id,
            &host.host_id,
            &host.boot_id,
            play.show.show.play_sequence,
        );
        qualified
            .require_source_scope(parent_plan, parent, &self.planned, &active, host)
            .map_err(error)?;
        let admission = play
            .instruction_admission
            .as_mut()
            .ok_or_else(|| error("instruction Source has no admission"))?;
        let value =
            play.scheduler
                .complete_instruction(admission, source_request, &qualified, host)?;
        let effect = play
            .instruction_effect
            .as_ref()
            .ok_or_else(|| error("instruction has no prepared effect operation"))?;
        let request = wait_request(&mut play.scheduler)?;
        if request.node != effect.node
            || request.call != effect.call
            || request.request != RequestId(0)
            || request.input.value != value
            || request.input.admitted_bytes != effect.binding.maximum_input_bytes
        {
            return Err(error(
                "instruction effect did not consume the original Source value",
            ));
        }
        admission
            .claim(source_request.node, source_request.call, value)
            .map_err(error)?;
        let completed = admission
            .acknowledge(source_request.node, source_request.call, value)
            .map_err(error)?;
        if completed.request() != source_request || completed.value() != request.input.value {
            return Err(error(
                "instruction effect custody differs from original Source completion",
            ));
        }
        admission
            .require_effect_origin(&completed, parent, &active, host)
            .map_err(error)?;
        let wording = admission.effect_wording(&completed).map_err(error)?;
        let prefix = b"\r\x1b[2K";
        let mut line = [0u8; 264];
        let length = prefix
            .len()
            .checked_add(wording.len())
            .filter(|length| *length <= line.len())
            .ok_or_else(|| error("instruction effect exceeds prepared line bound"))?;
        line[..prefix.len()].copy_from_slice(prefix);
        line[prefix.len()..length].copy_from_slice(wording.as_bytes());
        if &line[..length] != expected.as_bytes() || wording.len() > 256 {
            return Err(error(
                "instruction effect differs from qualified source wording",
            ));
        }
        output
            .write_all(&line[..length])
            .and_then(|()| output.flush())
            .map_err(|e| error(format!("instruction effect write/flush: {e}")))?;
        play.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(debug_error)?;
        // Consume the exact effect completion on the shared kernel before
        // transferring its custody back to the private input owner.
        for _ in 0..64 {
            if play
                .scheduler
                .instruction_effect_acknowledged(request.node)?
            {
                break;
            }
            step(&mut play.scheduler)?;
        }
        if !play
            .scheduler
            .instruction_effect_acknowledged(request.node)?
        {
            return Err(error(
                "instruction effect acknowledgement work bound exhausted",
            ));
        }
        admission
            .consume_acknowledged_effect(completed, parent, &active, host)
            .map_err(error)?;
        Ok(())
    }
}
