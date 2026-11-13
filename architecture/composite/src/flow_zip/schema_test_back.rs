//! Test-only Step dispatch for the actual Zip scheduler fixture.
use super::*;
pub(super) enum Back<'a> {
    Source {
        schema: &'a [u8],
        bytes: &'a [u8],
        separated: bool,
        count: u8,
        close_delay: u8,
        staged: bool,
    },
    Zip {
        back: FlowZipBack,
        separated: u8,
        canonical: u8,
    },
    Sink {
        schema: &'a [u8],
        expected: &'a [u8],
        separated: bool,
        delay: u8,
        count: u8,
        staged: bool,
    },
}
impl StepBack<2> for Back<'_> {
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Zip { back, .. } => {
                <FlowZipBack as StepBack<2>>::canonical_input_schema(back, port)
            }
            Self::Sink {
                schema,
                separated: true,
                ..
            } if port == PortId(0) => Some(schema),
            _ => None,
        }
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Zip { back, .. } => {
                <FlowZipBack as StepBack<2>>::canonical_output_schema(back, port)
            }
            Self::Source {
                schema,
                separated: true,
                ..
            } if port == PortId(0) => Some(schema),
            _ => None,
        }
    }
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; 2] {
        match self {
            Self::Zip { back, .. } => back.terminal_transductions(),
            _ => [None; 2],
        }
    }
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(&self) -> Option<StepWorkspaceRequirements> {
        if let Self::Zip { back, .. } = self {
            <FlowZipBack as StepBack<2>>::workspace_requirements(back)
        } else {
            None
        }
    }
    #[cfg(feature = "execution-workspace")]
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<2>,
        input: &StepInputBytes<'_, 2>,
        workspace: &mut StepWorkspace<'_>,
    ) -> StepOutcome {
        if let Self::Zip {
            back,
            separated,
            canonical,
        } = self
        {
            let result = back.step_with_workspace(io, input, workspace);
            for port in [PortId(0), PortId(1)] {
                if io.test_consumed(port) {
                    *separated += u8::from(input.canonical_parts(port).is_some());
                    *canonical += u8::from(input.input(port).is_some());
                }
            }
            result
        } else {
            self.step(io, input)
        }
    }
    fn step(&mut self, io: &mut StepIo<2>, input: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Source {
                bytes,
                count,
                close_delay,
                staged,
                ..
            } => {
                if *count == 2 {
                    // Keep the finite producers open until both pairs can form;
                    // shortest-stream closure may otherwise discard an unmatched member.
                    if *close_delay > 0 {
                        *close_delay -= 1;
                        io.exhaust_fuel();
                        return StepOutcome::Yield;
                    }
                    return StepOutcome::Complete;
                }
                if !io.output_ready_for(PortId(0), bytes.len() as u32) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), bytes.len() as u32).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Zip {
                back,
                separated,
                canonical,
            } => {
                let result = back.step(io, input);
                for port in [PortId(0), PortId(1)] {
                    if io.test_consumed(port) {
                        *separated += u8::from(input.canonical_parts(port).is_some());
                        *canonical += u8::from(input.input(port).is_some());
                    }
                }
                result
            }
            Self::Sink {
                schema,
                expected,
                separated,
                delay,
                staged,
                ..
            } => {
                if io.input(PortId(0)).is_none() {
                    if io.input_closed(PortId(0)) {
                        io.consume_closed(PortId(0)).unwrap();
                        return StepOutcome::Complete;
                    }
                    return StepOutcome::Await;
                }
                if *delay > 0 {
                    *delay -= 1;
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                if *separated {
                    let parts = input.canonical_parts(PortId(0)).unwrap();
                    assert_eq!(parts.schema, *schema);
                    assert_eq!(parts.node, &expected[schema.len()..]);
                    assert!(input.input(PortId(0)).is_none());
                } else {
                    assert_eq!(input.input(PortId(0)), Some(*expected));
                    assert!(input.canonical_parts(PortId(0)).is_none());
                }
                assert_eq!(
                    io.input(PortId(0)).unwrap().byte_len as usize,
                    expected.len()
                );
                io.consume(PortId(0)).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Source {
                bytes,
                staged: true,
                ..
            } if port == PortId(0) => Some(bytes),
            Self::Zip { back, .. } => <FlowZipBack as StepBack<2>>::prepared_output(back, port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { count, staged, .. } | Self::Sink { count, staged, .. } => {
                *count += u8::from(*staged);
                *staged = false;
            }
            Self::Zip { back, .. } => <FlowZipBack as StepBack<2>>::step_committed(back),
        }
    }
}
