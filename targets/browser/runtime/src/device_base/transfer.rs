use super::*;

impl BrowserSerialSession {
    pub(crate) fn begin_transfer(
        &mut self,
        direction: SerialTransferDirection,
    ) -> Result<(), BrowserSerialRefusal> {
        let BrowserSerialPhase::UsePlaying { resource, .. } = &self.phase else {
            return Err(BrowserSerialRefusal::WrongPhase);
        };
        if self.retained_transfer.is_some() {
            return Err(BrowserSerialRefusal::Pressure);
        }
        let admitted = match direction {
            SerialTransferDirection::Read => self.admitted_reads,
            SerialTransferDirection::Write => self.admitted_writes,
        };
        let maximum = match direction {
            SerialTransferDirection::Read => resource.transfer_bounds.maximum_reads,
            SerialTransferDirection::Write => resource.transfer_bounds.maximum_writes,
        };
        if admitted >= maximum {
            return Err(BrowserSerialRefusal::TransferLimit);
        }
        self.retained_transfer = Some((direction, 0));
        match direction {
            SerialTransferDirection::Read => self.admitted_reads += 1,
            SerialTransferDirection::Write => self.admitted_writes += 1,
        }
        Ok(())
    }

    pub(crate) fn complete_transfer(
        &mut self,
        direction: SerialTransferDirection,
        bytes: usize,
    ) -> Result<(), BrowserSerialRefusal> {
        let BrowserSerialPhase::UsePlaying { resource, .. } = &self.phase else {
            return Err(BrowserSerialRefusal::WrongPhase);
        };
        if self.retained_transfer != Some((direction, 0)) {
            return Err(BrowserSerialRefusal::WrongPhase);
        }
        if bytes == 0 || bytes > resource.transfer_bounds.maximum_transfer_bytes as usize {
            return Err(BrowserSerialRefusal::TransferTooLarge);
        }
        self.retained_transfer = Some((direction, bytes));
        Ok(())
    }

    pub(crate) fn release_transfer(&mut self) -> Result<(), BrowserSerialRefusal> {
        if self.retained_transfer.take().is_none() {
            return Err(BrowserSerialRefusal::WrongPhase);
        }
        Ok(())
    }

    pub(crate) fn fail_transfer(
        &mut self,
        terminal: BrowserSerialTerminal,
    ) -> Result<(), BrowserSerialRefusal> {
        if !matches!(self.phase, BrowserSerialPhase::UsePlaying { .. })
            || !matches!(
                terminal,
                BrowserSerialTerminal::TransferTooLarge
                    | BrowserSerialTerminal::TransferFailed
                    | BrowserSerialTerminal::ReadClosed
            )
        {
            return Err(BrowserSerialRefusal::WrongPhase);
        }
        self.retained_transfer = None;
        self.phase = BrowserSerialPhase::Terminal(terminal);
        Ok(())
    }
}
