//! Resident profile ownership behind Root's one carrier-independent supervisor.
use crate::{
    execution_supervisor::OwnedWorkset, resident::generated::ENTRY_COUNT,
    resident_preparation as r, transport::LIMIT,
};
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{scheduler::SchedulerError, KernelEvent};
pub struct ResidentWorkset<'w, 'o> {
    fragments: [Option<r::BoundFragment<'w, 'o>>; ENTRY_COUNT],
    terminal: [Option<r::TerminalFragment<'w>>; ENTRY_COUNT],
    stamp: b::RetainedHostCommandStamp,
    started: bool,
}
impl<'w, 'o> ResidentWorkset<'w, 'o> {
    pub fn new(
        fragments: [Option<r::BoundFragment<'w, 'o>>; ENTRY_COUNT],
        stamp: b::RetainedHostCommandStamp,
    ) -> Self {
        Self {
            fragments,
            terminal: core::array::from_fn(|_| None),
            stamp,
            started: false,
        }
    }
}
impl OwnedWorkset for ResidentWorkset<'_, '_> {
    type StartError = b::HostCommandRetentionRefusal;
    type StepError = SchedulerError;
    fn is_started(&self) -> bool {
        self.started
    }
    fn start(
        &mut self,
        input: &b::HostCommandInput<LIMIT>,
        context: b::NativePreparationContext<'_>,
        _: &c::ResourceAdmissionOwner,
    ) -> Result<(), Self::StartError> {
        let command = input.reborrow(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            b::HOST_PREPARED_WORKSET_CONTRACT,
            context.now_millis,
        )?;
        self.stamp.check_command(&command)?;
        self.started = true;
        Ok(())
    }
    fn step(&mut self) -> Result<bool, Self::StepError> {
        r::step_workset(&mut self.fragments)
    }
    fn retire(
        &mut self,
        owner: &mut c::ResourceAdmissionOwner,
        cause: c::ResourceReleaseCause,
    ) -> bool {
        r::retire_workset(&mut self.fragments, &mut self.terminal, owner, cause)
    }
    fn terminal_count(&self) -> usize {
        self.terminal.iter().flatten().count()
    }
    fn terminal_event(&self, fragment: usize, event: usize) -> Option<KernelEvent> {
        self.terminal
            .get(fragment)?
            .as_ref()?
            .retired
            .signs
            .events()
            .nth(event)
    }
}
