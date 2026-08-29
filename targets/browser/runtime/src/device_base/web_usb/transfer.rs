use super::*;

impl BrowserUsbSession {
    pub(crate) fn begin_transfer(
        &mut self,
        direction: UsbTransferDirection,
    ) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::UsePlaying { resource, .. } = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        if self.retained_transfer.is_some() {
            return Err(BrowserUsbRefusal::Pressure);
        }
        let (admitted, maximum) = match direction {
            UsbTransferDirection::In => (
                self.admitted_in_transfers,
                resource.transfer_bounds.maximum_in_transfers,
            ),
            UsbTransferDirection::Out => (
                self.admitted_out_transfers,
                resource.transfer_bounds.maximum_out_transfers,
            ),
        };
        if admitted >= maximum {
            return Err(BrowserUsbRefusal::TransferLimit);
        }
        self.retained_transfer = Some((direction, 0));
        match direction {
            UsbTransferDirection::In => self.admitted_in_transfers += 1,
            UsbTransferDirection::Out => self.admitted_out_transfers += 1,
        }
        Ok(())
    }

    pub(crate) fn complete_transfer(
        &mut self,
        direction: UsbTransferDirection,
        bytes: usize,
    ) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::UsePlaying { resource, .. } = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        if self.retained_transfer != Some((direction, 0)) {
            return Err(BrowserUsbRefusal::WrongPhase);
        }
        if bytes == 0 || bytes > resource.transfer_bounds.maximum_transfer_bytes as usize {
            return Err(BrowserUsbRefusal::TransferTooLarge);
        }
        self.retained_transfer = Some((direction, bytes));
        Ok(())
    }

    pub(crate) fn release_transfer(&mut self) -> Result<(), BrowserUsbRefusal> {
        self.retained_transfer
            .take()
            .map(|_| ())
            .ok_or(BrowserUsbRefusal::WrongPhase)
    }

    pub(crate) fn fail_transfer(
        &mut self,
        terminal: BrowserUsbTerminal,
    ) -> Result<(), BrowserUsbRefusal> {
        if !matches!(self.phase, BrowserUsbPhase::UsePlaying { .. })
            || !matches!(
                terminal,
                BrowserUsbTerminal::TransferTooLarge
                    | BrowserUsbTerminal::TransferStalled
                    | BrowserUsbTerminal::TransferBabble
                    | BrowserUsbTerminal::TransferFailed
            )
        {
            return Err(BrowserUsbRefusal::WrongPhase);
        }
        self.retained_transfer = None;
        self.phase = BrowserUsbPhase::Terminal(terminal);
        Ok(())
    }

    pub(crate) fn terminate_resource(
        &mut self,
        terminal: BrowserUsbTerminal,
    ) -> Result<(), BrowserUsbRefusal> {
        if !matches!(
            self.phase,
            BrowserUsbPhase::ResourceTruth(_)
                | BrowserUsbPhase::UsePlanned { .. }
                | BrowserUsbPhase::UsePlaying { .. }
        ) || !matches!(
            terminal,
            BrowserUsbTerminal::DeviceLost
                | BrowserUsbTerminal::CloseFailed
                | BrowserUsbTerminal::Closed
        ) {
            return Err(BrowserUsbRefusal::WrongPhase);
        }
        self.retained_transfer = None;
        self.phase = BrowserUsbPhase::Terminal(terminal);
        Ok(())
    }
}
