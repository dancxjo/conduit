//! Exact selector profiles; private output and caller output stay separate.
use super::*;
/// Pure checked selection runs as an ordinary StepBack, not a hardware Host Call.
pub struct SelectorBack {
    selector: PreparedCanonicalSelector,
    maximum: usize,
    staged: bool,
}
impl SelectorBack {
    pub fn prepare(
        selector: conduit_core::StructuredSelector,
        input: usize,
        output: usize,
    ) -> Result<Self, &'static str> {
        if input == 0
            || output == 0
            || input > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || output > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("selector-bound");
        }
        Ok(Self {
            selector: PreparedCanonicalSelector::prepare(selector, input, output)
                .map_err(|_| "selector")?,
            maximum: output,
            staged: false,
        })
    }
    pub fn prepare_static(
        canonical: &'static [u8],
        input: usize,
        output: usize,
    ) -> Result<Self, &'static str> {
        if input == 0
            || output == 0
            || input > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || output > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("selector-bound");
        }
        Ok(Self {
            selector: PreparedCanonicalSelector::prepare_static(canonical, input, output)
                .map_err(|_| "selector")?,
            maximum: output,
            staged: false,
        })
    }
}
impl<const P: usize> StepBack<P> for SelectorBack {
    #[cfg(feature = "schema-value-storage")]
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.selector.input_schema_bytes())
            .flatten()
    }
    #[cfg(feature = "schema-value-storage")]
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.selector.output_schema_bytes())
            .flatten()
    }

    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(1);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_ok() {
                    StepOutcome::Complete
                } else {
                    fail(2)
                }
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let canonical = input.input(PortId(0));
        #[cfg(feature = "schema-value-storage")]
        let parts = input.canonical_parts(PortId(0));
        #[cfg(not(feature = "schema-value-storage"))]
        let parts: Option<conduit_kernel::CanonicalValueParts<'_>> = None;
        let length = match (canonical, parts) {
            (Some(bytes), None) => Some(bytes.len()),
            (None, Some(parts)) => parts.schema.len().checked_add(parts.node.len()),
            _ => None,
        };
        if length != Some(reference.byte_len as usize) || io.consume_fuel(1).is_err() {
            return fail(4);
        }
        let selected = match (canonical, parts) {
            (Some(bytes), None) => self.selector.select(bytes),
            (None, Some(parts)) => self.selector.select_parts(parts.schema, parts.node),
            _ => return fail(3),
        };
        match selected {
            Ok(Some(value)) if value.len() <= self.maximum => {
                if io.send_prepared(PortId(0), value.len() as u32).is_err() {
                    return fail(5);
                }
                self.staged = true;
            }
            Ok(None) => {}
            _ => return fail(6),
        }
        if io.consume(PortId(0)).is_err() {
            return fail(7);
        }
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(self.selector.output())
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.staged = false;
    }
}

#[cfg(feature = "execution-workspace")]
pub struct WorkspaceSelectorBack {
    selector: conduit_semantic_catalog::PreparedLinkedWorkspaceSelector,
    staged: bool,
}
#[cfg(feature = "execution-workspace")]
impl WorkspaceSelectorBack {
    pub fn prepare_static(
        canonical: &'static [u8],
        input: usize,
        output: usize,
    ) -> Result<Self, &'static str> {
        if input == 0
            || output == 0
            || input > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || output > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("selector-bound");
        }
        Ok(Self {
            selector: conduit_semantic_catalog::PreparedLinkedWorkspaceSelector::prepare_static(
                canonical, input, output,
            )
            .map_err(|_| "selector")?,
            staged: false,
        })
    }
    fn run<const P: usize>(
        &mut self,
        io: &mut StepIo<P>,
        input: &StepInputBytes<'_, P>,
        output: Option<&mut [u8]>,
    ) -> StepOutcome {
        self.staged = false;
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(1);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_ok() {
                    StepOutcome::Complete
                } else {
                    fail(2)
                }
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let canonical = input.input(PortId(0));
        #[cfg(feature = "schema-value-storage")]
        let parts = input.canonical_parts(PortId(0));
        #[cfg(not(feature = "schema-value-storage"))]
        let parts: Option<conduit_kernel::CanonicalValueParts<'_>> = None;
        let length = match (canonical, parts) {
            (Some(bytes), None) => Some(bytes.len()),
            (None, Some(parts)) => parts.schema.len().checked_add(parts.node.len()),
            _ => None,
        };
        if length != Some(reference.byte_len as usize) || io.consume_fuel(1).is_err() {
            return fail(4);
        }
        let Some(output) = output else {
            return fail(6);
        };
        let selected = match (canonical, parts) {
            (Some(bytes), None) => self.selector.select_into(bytes, output),
            (None, Some(parts)) => {
                self.selector
                    .select_parts_into(parts.schema, parts.node, output)
            }
            _ => return fail(3),
        };
        match selected {
            Ok(Some(length)) => {
                if !io.output_ready_for(PortId(0), length as u32) {
                    return StepOutcome::Await;
                }
                if io.send_workspace_output(PortId(0), length as u32).is_err() {
                    return fail(5);
                }
                self.staged = true;
            }
            Ok(None) => {}
            Err(_) => return fail(6),
        }
        if io.consume(PortId(0)).is_err() {
            return fail(7);
        }
        StepOutcome::Progress
    }
}
#[cfg(feature = "execution-workspace")]
impl<const P: usize> StepBack<P> for WorkspaceSelectorBack {
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        Some(conduit_kernel::scheduler::StepWorkspaceRequirements {
            output_bytes: self.selector.maximum_output_bytes() as u32,
            scratch_bytes: 0,
        })
    }
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<P>,
        input: &StepInputBytes<'_, P>,
        workspace: &mut conduit_kernel::scheduler::StepWorkspace<'_>,
    ) -> StepOutcome {
        let (output, _) = workspace.buffers();
        self.run(io, input, Some(output))
    }
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.run(io, input, None)
    }
    #[cfg(feature = "schema-value-storage")]
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.selector.input_schema_bytes())
    }
    #[cfg(feature = "schema-value-storage")]
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.selector.output_schema_bytes())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
    }
}

#[cfg(all(test, feature = "execution-workspace"))]
#[path = "execution_selector_workspace_tests.rs"]
mod workspace_tests;
