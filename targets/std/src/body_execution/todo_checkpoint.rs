//! Selected Todo residence routes for one ordinary Body Play.

use super::{
    BodyForeExchange, BodyForeOutputAdapter, BodyRunReport, BodyRunRequest, LiveForeTerminalGuard,
    TodoCheckpointSelection,
};
use crate::{BodyLiveForeQueue, StdHost, TimerAdapter};
use conduit_body::{BodyPlayIdentity, Wake};
use std::{io::Write, path::Path};

impl StdHost {
    /// Run one exact Todo command with a selected checkpoint residence. The
    /// committed Fore state is available only after its admitted Host Call.
    pub fn run_body_plan_with_todo_checkpoint_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        fore: BodyForeExchange<'_>,
        checkpoint: TodoCheckpointSelection<'_>,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_with_todo_checkpoint_to_with_start(
            request,
            fore,
            checkpoint,
            output,
            timer,
            |_, _| Ok(()),
        )
    }

    /// Retain the exact started Play before any checkpoint Host Call can
    /// publish state. A refused start leaves the selected residence untouched.
    pub fn run_body_plan_with_todo_checkpoint_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        fore: BodyForeExchange<'_>,
        checkpoint: TodoCheckpointSelection<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.require_selected_todo_checkpoint_root(checkpoint.root)?;
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            Some((fore.inputs, false, fore.output)),
            None,
            Some((checkpoint.root, checkpoint.identity)),
            started,
        )
    }

    /// Restore one typed published state through its selected read Host Call.
    /// The start callback runs before any external read or Fore delivery.
    pub fn run_body_plan_with_todo_checkpoint_read_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        fore_output: &mut dyn BodyForeOutputAdapter,
        checkpoint: TodoCheckpointSelection<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.require_selected_todo_checkpoint_root(checkpoint.root)?;
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            Some((&[], false, fore_output)),
            None,
            Some((checkpoint.root, checkpoint.identity)),
            started,
        )
    }

    fn require_selected_todo_checkpoint_root(&self, root: &Path) -> Result<(), String> {
        let selected = self
            .todo_checkpoint_root
            .as_ref()
            .ok_or("std Host has no selected Todo checkpoint residence")?;
        if root
            .symlink_metadata()
            .map_err(|error| format!("Todo checkpoint root: {error}"))?
            .file_type()
            .is_symlink()
        {
            return Err("Todo checkpoint root changed to a symlink".into());
        }
        let actual = root
            .canonicalize()
            .map_err(|error| format!("Todo checkpoint root: {error}"))?;
        if actual != selected.path {
            return Err("Todo checkpoint root differs from advertised residence".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = actual
                .metadata()
                .map_err(|error| format!("Todo checkpoint root: {error}"))?;
            if metadata.dev() != selected.device || metadata.ino() != selected.inode {
                return Err("Todo checkpoint residence was rebound after advertisement".into());
            }
        }
        Ok(())
    }

    /// Wait for one later typed command in the same ordinary Body Play. The
    /// selected current state is already bound to the queue's exact Plan; the
    /// publisher is attached before start and no output precedes its Host Call.
    pub fn run_body_plan_with_waiting_todo_checkpoint_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        queue: &BodyLiveForeQueue,
        fore_output: &mut dyn BodyForeOutputAdapter,
        checkpoint: TodoCheckpointSelection<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let _live_guard = LiveForeTerminalGuard(Some(queue));
        if queue.initial().is_none() {
            return Err("waiting checkpoint requires an admitted current state".into());
        }
        self.require_selected_todo_checkpoint_root(checkpoint.root)?;
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            None,
            Some((queue, fore_output)),
            Some((checkpoint.root, checkpoint.identity)),
            started,
        )
    }
}
