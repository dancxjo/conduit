//! Normal completion comes from exact Cord terminals, never a response count.
use super::*;
fn completed(message: OwnedMessage, sequence: u64) -> Result<()> {
    if matches!(message, OwnedMessage::Terminal(SessionTerminalDisposition::Completed, s) if s == sequence)
    {
        Ok(())
    } else {
        Err("history Cord omitted exact completed terminal".into())
    }
}
impl<C: ProtectedFrameCarrier> HistoryRelay<C> {
    /// The collector closes its request Cord, acknowledges the response Cord,
    /// then closes the outer carrier only after both machines are terminal.
    pub(crate) fn close(&mut self) -> Result<()> {
        if !self.collector {
            return Err("history source must await typed collector close".into());
        }
        let request = self.request.next_sequence();
        let response = self.response.next_sequence();
        send(
            &mut self.line,
            &mut self.request,
            &mut self.frame,
            SessionMessage::InputClosed {
                final_sequence: request,
            },
        )?;
        send(
            &mut self.line,
            &mut self.request,
            &mut self.frame,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: request,
            },
        )?;
        if !matches!(receive(&mut self.line,&mut self.response)?,OwnedMessage::InputClosed(s) if s == response)
        {
            return Err("history response omitted exact input close".into());
        }
        completed(receive(&mut self.line, &mut self.response)?, response)?;
        send(
            &mut self.line,
            &mut self.response,
            &mut self.frame,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: response,
            },
        )?;
        completed(receive(&mut self.line, &mut self.request)?, request)?;
        if !self.request.is_terminal() || !self.response.is_terminal() {
            return Err("history collector Cord closure is incomplete".into());
        }
        checked(self.line.close())
    }
    /// Call only after serve_one reports InputClosed and the source kernel has
    /// consumed its exact input close and drained. Two-frame bursts remain the
    /// maximum; no added queue or timing delay supplies hidden credit.
    pub(crate) fn finish_source(&mut self) -> Result<()> {
        if self.collector {
            return Err("collector cannot finish source Cords".into());
        }
        let request = self.request.next_sequence();
        let response = self.response.next_sequence();
        completed(receive(&mut self.line, &mut self.request)?, request)?;
        send(
            &mut self.line,
            &mut self.response,
            &mut self.frame,
            SessionMessage::InputClosed {
                final_sequence: response,
            },
        )?;
        send(
            &mut self.line,
            &mut self.response,
            &mut self.frame,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: response,
            },
        )?;
        completed(receive(&mut self.line, &mut self.response)?, response)?;
        send(
            &mut self.line,
            &mut self.request,
            &mut self.frame,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: request,
            },
        )?;
        if !self.request.is_terminal() || !self.response.is_terminal() {
            return Err("history source Cord closure is incomplete".into());
        }
        // Only the collector initiates outer close, after receiving our last
        // terminal. EOF, timeout and network loss remain failures even here.
        match self.line.receive() {
            Err(conduit_protected_line::ProtectedLineError::Closed) => Ok(()),
            Err(reason) => {
                Err(format!("history terminal carrier did not close explicitly: {reason:?}").into())
            }
            Ok(_) => Err("history carrier sent a late frame after both terminals".into()),
        }
    }
}
