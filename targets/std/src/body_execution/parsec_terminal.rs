//! Explicit terminal effect attachment for the bounded Parsec Body profile.
use super::*;
use conduit_presentation::{MaskShow, Presentation};

pub trait BodyParsecShowObserver {
    /// Inspect actual Show evidence. Observing it grants no input authority.
    fn showed(&mut self, face: &Presentation, show: &MaskShow) -> Result<(), String>;
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
    pub output: &'a mut dyn Write,
    pub observer: &'a mut dyn BodyParsecShowObserver,
    pub host: conduit_core::HostAdvertisement,
    pub columns: usize,
    pub rows: usize,
}

impl StdHost {
    /// Attach an explicitly supplied terminal writer to one exact Parsec Play.
    /// The current Host must offer the terminal Mask. This renders source-derived
    /// Faces and acknowledged Shows; device-to-action binding remains separate.
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
