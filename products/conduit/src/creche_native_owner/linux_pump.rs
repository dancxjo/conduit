//! One bounded UI turn; Body execution and make/save effects remain external.
use super::{LocalCrecheNativeOwner, LocalCrecheScanout};
use crate::creche_encounter::CrecheEncounterController;
use conduit_linux_display::{EventPump, NativeInput, NativeInputRefusal, Scanout};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinuxCrecheTurn {
    Idle,
    QueuePending,
    AwaitingPresentation,
    AwaitingConsumption,
    InputQueued,
    Quit,
    Retired,
}
#[derive(Debug, Eq, PartialEq)]
pub enum LinuxCrechePumpError {
    Input(NativeInputRefusal),
    Owner(String),
}
impl LocalCrecheNativeOwner<&mut Scanout<'_>> {
    /// Service retirement, publish the actual current Face, then acquire one
    /// observation through this same live output. A semantic Face replacement
    /// drains old queued keys before selection. At most two driver operations
    /// inspect `maximum_events` entries each (1..=256); this call never waits for a Body worker.
    /// Quit drains presentation only. The caller must cancel the Body worker,
    /// keep servicing its retirement handshake, then collect its actual outcome.
    pub fn pump_linux(
        &mut self,
        controller: &CrecheEncounterController,
        events: &mut EventPump,
        maximum_events: u16,
    ) -> Result<LinuxCrecheTurn, LinuxCrechePumpError> {
        self.pump_input_turn(controller, maximum_events, |display, withdraw, bound| {
            if withdraw {
                display.withdraw_input();
            }
            match display.prepare_input(events, bound)? {
                NativeInput::Idle => display.poll_input(events, bound),
                other => Ok(other),
            }
        })
    }
}
impl<D: LocalCrecheScanout> LocalCrecheNativeOwner<D> {
    // Private seam for deterministic protocol tests using explicit CPU output;
    // the public Linux entrance always uses the actual retained SDL scanout.
    pub(super) fn pump_input_turn(
        &mut self,
        controller: &CrecheEncounterController,
        maximum_events: u16,
        mut acquire: impl FnMut(&mut D, bool, u16) -> Result<NativeInput, NativeInputRefusal>,
    ) -> Result<LinuxCrecheTurn, LinuxCrechePumpError> {
        use LinuxCrechePumpError as E;
        if self.shared.is_drained().map_err(E::Owner)?
            || self.service_retirement().map_err(E::Owner)?
        {
            return Ok(LinuxCrecheTurn::Retired);
        }
        if maximum_events == 0 || maximum_events > 256 {
            return Err(E::Input(NativeInputRefusal::InvalidBound));
        }
        let published =
            self.execution.is_some() && self.present_current(controller).map_err(E::Owner)?;
        if self.shared.is_drained().map_err(E::Owner)? {
            return Ok(LinuxCrecheTurn::Retired);
        }
        let available = self.execution.is_some() && self.mask.show().is_some();
        let waiting = self.shared.has_returned().map_err(E::Owner)?;
        let observation = match acquire(&mut self.display, published || !available, maximum_events)
        {
            Ok(input) => input,
            Err(error) => {
                if error == NativeInputRefusal::Lost {
                    self.retire().map_err(E::Owner)?;
                }
                return Err(E::Input(error));
            }
        };
        match observation {
            NativeInput::Quit => {
                self.retire().map_err(E::Owner)?;
                Ok(LinuxCrecheTurn::Quit)
            }
            NativeInput::Pending => Ok(LinuxCrecheTurn::QueuePending),
            _ if waiting => Ok(LinuxCrecheTurn::AwaitingConsumption),
            _ if !available => Ok(LinuxCrecheTurn::AwaitingPresentation),
            NativeInput::Idle => Ok(LinuxCrecheTurn::Idle),
            NativeInput::Key(event) => {
                self.key(controller, event)
                    .map_err(E::Owner)
                    .map(|submitted| {
                        if submitted {
                            LinuxCrecheTurn::InputQueued
                        } else {
                            LinuxCrecheTurn::Idle
                        }
                    })
            }
        }
    }
}
