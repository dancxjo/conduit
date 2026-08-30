use super::*;

impl BrowserUsbSession {
    pub(crate) fn begin_transfer(
        &mut self,
        kind: UsbTransferKind,
        direction: UsbTransferDirection,
        control_setup: Option<UsbControlSetup>,
    ) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::UsePlaying { resource, .. } = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        if self.retained_transfer.is_some() {
            return Err(BrowserUsbRefusal::Pressure);
        }
        if matches!(kind, UsbTransferKind::Bulk) != control_setup.is_none() {
            return Err(BrowserUsbRefusal::WrongPhase);
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
        self.retained_transfer = Some(RetainedUsbTransfer {
            kind,
            direction,
            control_setup,
            completed_bytes: None,
        });
        match direction {
            UsbTransferDirection::In => self.admitted_in_transfers += 1,
            UsbTransferDirection::Out => self.admitted_out_transfers += 1,
        }
        Ok(())
    }

    pub(crate) fn complete_transfer(
        &mut self,
        kind: UsbTransferKind,
        direction: UsbTransferDirection,
        bytes: usize,
    ) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::UsePlaying { resource, .. } = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        let Some(retained) = self.retained_transfer.as_mut() else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        if retained.kind != kind
            || retained.direction != direction
            || retained.completed_bytes.is_some()
        {
            return Err(BrowserUsbRefusal::WrongPhase);
        }
        if bytes > resource.transfer_bounds.maximum_transfer_bytes as usize {
            return Err(BrowserUsbRefusal::TransferTooLarge);
        }
        retained.completed_bytes = Some(bytes);
        Ok(())
    }

    pub(crate) fn release_transfer(&mut self) -> Result<(), BrowserUsbRefusal> {
        match self.retained_transfer {
            Some(RetainedUsbTransfer {
                completed_bytes: Some(_),
                ..
            }) => {
                self.retained_transfer = None;
                Ok(())
            }
            _ => Err(BrowserUsbRefusal::WrongPhase),
        }
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
