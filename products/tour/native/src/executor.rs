use std::io::Write;

use conduit_tour_model::{
    TOUR_CHAPTERS, TourComparisonProof, TourRunProof, TourRunTerminal, TourStageMode,
    tour_stage_source,
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
        // Multi-Host stages stay explicitly unsupported until their exact
        // secondary fragments, Plays, and planned Line are retained here.
        let supported = matches!(stage_contract.mode, TourStageMode::Run)
            && matches!(chapter, 0 | 2)
            || matches!(stage_contract.mode, TourStageMode::Compare) && chapter == 1;
        if !supported {
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
        let mut comparison = None;
        let (execution, output, terminal, result_is_verified) = if stage_contract.mode
            == TourStageMode::Compare
        {
            let direct_control = conduit_std_host::RunControl::default();
            let mut direct_output = StopAfterManifestations::new(&direct_control, 1);
            let direct = conduit::execute_hosted_form_controlled(
                &source,
                &mut direct_output,
                &direct_control,
            )
            .map_err(HostedTourExecutorRefusal::Execution)?;
            let recursive_control = conduit_std_host::RunControl::default();
            let mut recursive_output = StopAfterManifestations::new(&recursive_control, 1);
            let recursive = conduit::execute_hosted_form_recursive_controlled(
                &source,
                &mut recursive_output,
                &recursive_control,
            )
            .map_err(HostedTourExecutorRefusal::Execution)?;
            let direct_rendered = core::str::from_utf8(&direct_output.output.bytes)
                .map_err(|_| HostedTourExecutorRefusal::WrongResult)?;
            let recursive_rendered = core::str::from_utf8(&recursive_output.output.bytes)
                .map_err(|_| HostedTourExecutorRefusal::WrongResult)?;
            let direct_manifestations = manifestation_lines(direct_rendered);
            let recursive_manifestations = manifestation_lines(recursive_rendered);
            if !direct_output.stop_requested
                || !recursive_output.stop_requested
                || direct_manifestations.len() + recursive_manifestations.len()
                    != usize::from(expected_manifestations)
                || direct_manifestations != recursive_manifestations
                || direct.source_document_id != recursive.source_document_id
                || direct.checked_form_id != recursive.checked_form_id
                || direct.expanded_form_id == recursive.expanded_form_id
                || direct.plan.plan_id == recursive.plan.plan_id
            {
                return Err(HostedTourExecutorRefusal::WrongResult);
            }
            comparison = Some(TourComparisonProof {
                expanded_form_id: recursive.expanded_form_id,
                plan_id: recursive.plan.plan_id,
            });
            (
                direct,
                direct_output.output.bytes,
                TourRunTerminal::Stopped,
                true,
            )
        } else if chapter == 0 && stage == 0 {
            let mut output = BoundedEvidence::new();
            let execution = conduit::execute_hosted_form(&source, &mut output)
                .map_err(HostedTourExecutorRefusal::Execution)?;
            (execution, output.bytes, TourRunTerminal::Completed, false)
        } else {
            let control = conduit_std_host::RunControl::default();
            let mut output =
                StopAfterEvidence::new(&control, expected, usize::from(expected_manifestations));
            let execution = conduit::execute_hosted_form_controlled(&source, &mut output, &control)
                .map_err(HostedTourExecutorRefusal::Execution)?;
            if !output.stop_requested {
                return Err(HostedTourExecutorRefusal::WrongTerminal);
            }
            (
                execution,
                output.output.bytes,
                TourRunTerminal::Stopped,
                false,
            )
        };
        let rendered =
            core::str::from_utf8(&output).map_err(|_| HostedTourExecutorRefusal::WrongResult)?;
        if !result_is_verified && !has_expected_result(rendered, expected) {
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
            comparison,
            multi_host: None,
        })
    }
}

struct StopAfterManifestations<'a> {
    control: &'a conduit_std_host::RunControl,
    expected_manifestations: usize,
    output: BoundedEvidence,
    stop_requested: bool,
}

impl<'a> StopAfterManifestations<'a> {
    fn new(control: &'a conduit_std_host::RunControl, expected_manifestations: usize) -> Self {
        Self {
            control,
            expected_manifestations,
            output: BoundedEvidence::new(),
            stop_requested: false,
        }
    }

    fn maybe_stop(&mut self) {
        let Ok(rendered) = core::str::from_utf8(&self.output.bytes) else {
            return;
        };
        if !self.stop_requested && manifestation_count(rendered) >= self.expected_manifestations {
            self.stop_requested = self
                .control
                .request_stop(
                    conduit_std_host::RunControlRequestId::new("tour/native/comparison-complete")
                        .expect("static hosted Tour control identity is valid"),
                )
                .is_ok();
        }
    }
}

impl Write for StopAfterManifestations<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.output.write_all(bytes)?;
        self.maybe_stop();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
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
        let manifestations = manifestation_count(rendered);
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

fn manifestation_count(rendered: &str) -> usize {
    manifestation_lines(rendered).len()
}

fn manifestation_lines(rendered: &str) -> Vec<&str> {
    rendered
        .lines()
        .filter(|line| {
            line.starts_with("PRESENTATION-TEXT")
                || line.starts_with("indicator unit-ms=")
                || line.starts_with("count value=")
        })
        .collect()
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
    fn comparison_retains_distinct_recursive_form_and_plan_with_equal_output() {
        let proof = HostedTourExecutor::run(1, 0).unwrap();
        let recursive = proof.comparison.unwrap();
        assert_ne!(proof.expanded_form_id, recursive.expanded_form_id);
        assert_ne!(proof.plan_id, recursive.plan_id);
        assert_eq!(proof.result, "Direct and recursive realizations agree");
        assert_eq!(proof.terminal, TourRunTerminal::Stopped);
    }

    #[test]
    fn distinct_unimplemented_hosted_lifecycles_do_not_gain_fake_success() {
        assert_eq!(
            HostedTourExecutor::run(4, 0),
            Err(HostedTourExecutorRefusal::UnknownStage)
        );
    }
}
