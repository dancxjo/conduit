use std::io::Write;

use conduit_tour_model::{
    TOUR_CHAPTERS, TourRunProof, TourRunTerminal, TourStageMode, tour_stage_source,
};

const HOSTED_EVIDENCE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostedTourExecutorRefusal {
    UnknownStage,
    UnsupportedStage,
    Execution(String),
    WrongResult,
    MissingIdentity,
    WrongTerminal,
}

/// Hosted execution adapter for authority-bearing requests from the shared Tour port.
pub struct HostedTourExecutor;

impl HostedTourExecutor {
    pub fn run(chapter: u8, stage: u8) -> Result<TourRunProof, HostedTourExecutorRefusal> {
        let stage_contract = TOUR_CHAPTERS
            .get(usize::from(chapter))
            .and_then(|chapter| chapter.stages.get(usize::from(stage)))
            .ok_or(HostedTourExecutorRefusal::UnknownStage)?;
        // Comparison and multi-Host stages stay explicitly unsupported until
        // their distinct secondary-Plan/Line evidence is retained here.
        if stage_contract.mode != TourStageMode::Run || !matches!(chapter, 0 | 2) {
            return Err(HostedTourExecutorRefusal::UnsupportedStage);
        }
        let source = tour_stage_source(chapter, stage)
            .map_err(|_| HostedTourExecutorRefusal::UnknownStage)?;
        let expected = stage_contract
            .expected_text
            .ok_or(HostedTourExecutorRefusal::WrongResult)?;
        let expected_manifestations = stage_contract
            .expected_manifestations
            .ok_or(HostedTourExecutorRefusal::WrongTerminal)?;
        let (execution, output, terminal) = if chapter == 0 && stage == 0 {
            let mut output = BoundedEvidence::new();
            let execution = conduit::execute_hosted_form(&source, &mut output)
                .map_err(HostedTourExecutorRefusal::Execution)?;
            (execution, output.bytes, TourRunTerminal::Completed)
        } else {
            let control = conduit_std_host::RunControl::default();
            let mut output =
                StopAfterEvidence::new(&control, expected, usize::from(expected_manifestations));
            let execution = conduit::execute_hosted_form_controlled(&source, &mut output, &control)
                .map_err(HostedTourExecutorRefusal::Execution)?;
            if !output.stop_requested {
                return Err(HostedTourExecutorRefusal::WrongTerminal);
            }
            (execution, output.output.bytes, TourRunTerminal::Stopped)
        };
        let rendered =
            core::str::from_utf8(&output).map_err(|_| HostedTourExecutorRefusal::WrongResult)?;
        if !has_expected_result(rendered, expected) {
            return Err(HostedTourExecutorRefusal::WrongResult);
        }
        let active_play_id = execution
            .observations
            .iter()
            .find_map(|observation| observation.active_play_id.clone())
            .ok_or(HostedTourExecutorRefusal::MissingIdentity)?;
        let exact_terminal = execution
            .observations
            .iter()
            .any(|observation| match terminal {
                TourRunTerminal::Completed => matches!(
                    observation.kind,
                    conduit_core::ObservationKind::PlanCompleted
                        | conduit_core::ObservationKind::PlanTerminal {
                            disposition: conduit_core::TerminalDisposition::Completed
                        }
                ),
                TourRunTerminal::Stopped => matches!(
                    observation.kind,
                    conduit_core::ObservationKind::Cancelled
                        | conduit_core::ObservationKind::PlanTerminal {
                            disposition: conduit_core::TerminalDisposition::Cancelled { .. }
                        }
                ),
            });
        if !exact_terminal {
            return Err(HostedTourExecutorRefusal::WrongTerminal);
        }
        Ok(TourRunProof {
            specimen_id: stage_contract.identity.into(),
            source_document_id: execution.source_document_id,
            checked_form_id: execution.checked_form_id,
            expanded_form_id: execution.expanded_form_id,
            plan_id: execution.plan.plan_id,
            active_play_id,
            result: expected.into(),
            terminal,
            comparison: None,
            multi_host: None,
        })
    }
}

struct StopAfterEvidence<'a> {
    control: &'a conduit_std_host::RunControl,
    expected: &'a str,
    expected_manifestations: usize,
    output: BoundedEvidence,
    stop_requested: bool,
}

impl<'a> StopAfterEvidence<'a> {
    fn new(
        control: &'a conduit_std_host::RunControl,
        expected: &'a str,
        expected_manifestations: usize,
    ) -> Self {
        Self {
            control,
            expected,
            expected_manifestations,
            output: BoundedEvidence::new(),
            stop_requested: false,
        }
    }

    fn maybe_stop(&mut self) {
        let Ok(rendered) = core::str::from_utf8(&self.output.bytes) else {
            return;
        };
        let has_result = has_expected_result(rendered, self.expected);
        let manifestations = rendered
            .lines()
            .filter(|line| {
                line.starts_with("PRESENTATION-TEXT")
                    || line.starts_with("indicator unit-ms=")
                    || line.starts_with("count value=")
            })
            .count();
        if !self.stop_requested && has_result && manifestations >= self.expected_manifestations {
            self.stop_requested = self
                .control
                .request_stop(
                    conduit_std_host::RunControlRequestId::new("tour/native/exercise-complete")
                        .expect("static hosted Tour control identity is valid"),
                )
                .is_ok();
        }
    }
}

fn has_expected_result(rendered: &str, expected: &str) -> bool {
    rendered
        .lines()
        .any(|line| line == expected || line.strip_prefix("count value=") == Some(expected))
}

impl Write for StopAfterEvidence<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.output.write_all(bytes)?;
        self.maybe_stop();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct BoundedEvidence {
    bytes: Vec<u8>,
}

impl BoundedEvidence {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(HOSTED_EVIDENCE_BYTES),
        }
    }
}

impl Write for BoundedEvidence {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > HOSTED_EVIDENCE_BYTES {
            return Err(std::io::Error::other(
                "hosted Tour evidence exceeded its admitted byte capacity",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_native_exercise_crosses_the_real_hosted_plan_and_play() {
        let proof = HostedTourExecutor::run(0, 0).unwrap();
        assert_eq!(proof.specimen_id, "canonical-form:meet-one-gear");
        assert_eq!(proof.result, "HELLO");
        assert!(!proof.plan_id.as_str().is_empty());
        assert!(!proof.active_play_id.as_str().is_empty());
        assert_eq!(proof.terminal, TourRunTerminal::Completed);
    }

    #[test]
    fn standing_and_timer_exercises_stop_only_after_their_exact_evidence() {
        for stage in 1..=2 {
            let proof = HostedTourExecutor::run(0, stage).unwrap();
            assert_eq!(proof.terminal, TourRunTerminal::Stopped);
            assert!(!proof.active_play_id.as_str().is_empty());
        }
        let timed = HostedTourExecutor::run(2, 0).unwrap();
        assert_eq!(timed.result, "1");
        assert_eq!(timed.terminal, TourRunTerminal::Stopped);
    }

    #[test]
    fn distinct_unimplemented_hosted_lifecycles_do_not_gain_fake_success() {
        assert_eq!(
            HostedTourExecutor::run(1, 0),
            Err(HostedTourExecutorRefusal::UnsupportedStage)
        );
        assert_eq!(
            HostedTourExecutor::run(4, 0),
            Err(HostedTourExecutorRefusal::UnknownStage)
        );
    }
}
