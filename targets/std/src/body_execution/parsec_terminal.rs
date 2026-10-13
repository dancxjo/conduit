//! Explicit terminal effect attachment for the bounded Parsec Body profile.
use super::*;
use conduit_presentation::{MaskShow, Presentation};

pub trait BodyParsecShowObserver {
    /// Inspect actual Show evidence. Observing it grants no input authority.
    fn showed(&mut self, face: &Presentation, show: &MaskShow) -> Result<(), String>;
    /// Observe a privately verified, flushed instruction. Metadata is not an
    /// input grant and says nothing about another medium's Show or physical input.
    fn instruction_observed(
        &mut self,
        _: &ParsecInputInstructionObservation,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub struct BodyParsecTerminalRoute<'a> {
    pub queue: &'a BodyLiveForeQueue,
    pub fore_output: &'a mut dyn BodyForeOutputAdapter,
    pub terminal_output: &'a mut dyn Write,
    pub observer: &'a mut dyn BodyParsecShowObserver,
    pub columns: usize,
    pub rows: usize,
}

pub(crate) struct ParsecTerminalAttachment<'a> {
    pub queue: &'a BodyLiveForeQueue,
    pub output: &'a mut dyn Write,
    pub observer: &'a mut dyn BodyParsecShowObserver,
    pub host: conduit_core::HostAdvertisement,
    pub columns: usize,
    pub rows: usize,
}

impl StdHost {
    /// Attach an explicitly supplied terminal writer to one exact Parsec Play.
    /// The current Host must offer the terminal Mask. Completed admitted input
    /// is checked against the current source map and acknowledged Face/Show,
    /// returned through the concrete Mask, then staged as a typed source command.
    pub fn run_body_plan_with_parsec_terminal_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        route: BodyParsecTerminalRoute<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let terminal = ParsecTerminalAttachment {
            queue: route.queue,
            output: route.terminal_output,
            observer: route.observer,
            host: self.advertisement.clone(),
            columns: route.columns,
            rows: route.rows,
        };
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            None,
            Some((route.queue, route.fore_output)),
            None,
            Some(terminal),
            started,
        )
    }
}
