use conduit_tour_model::{
    TOUR_CHAPTERS, TourRunProof, TourRunTerminal, TourStageMode, tour_stage_source,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedTourExecutorRefusal {
    UnknownStage,
    UnsupportedStage,
    Execution,
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
        // The first finite pipeline is admitted now. Standing, comparison, and
        // multi-Host stages stay explicitly unsupported until their distinct
        // Stop/secondary-Plan/Line evidence is retained by this Host adapter.
        if chapter != 0 || stage != 0 || stage_contract.mode != TourStageMode::Run {
            return Err(HostedTourExecutorRefusal::UnsupportedStage);
        }
        let source = tour_stage_source(chapter, stage)
            .map_err(|_| HostedTourExecutorRefusal::UnknownStage)?;
        let expected = stage_contract
            .expected_text
            .ok_or(HostedTourExecutorRefusal::WrongResult)?;
        let mut output = Vec::new();
        let execution = conduit::execute_hosted_form(&source, &mut output)
            .map_err(|_| HostedTourExecutorRefusal::Execution)?;
        let rendered =
            core::str::from_utf8(&output).map_err(|_| HostedTourExecutorRefusal::WrongResult)?;
        if !rendered.lines().any(|line| line == expected) {
            return Err(HostedTourExecutorRefusal::WrongResult);
        }
        let active_play_id = execution
            .observations
            .iter()
            .find_map(|observation| observation.active_play_id.clone())
            .ok_or(HostedTourExecutorRefusal::MissingIdentity)?;
        let completed = execution.observations.iter().any(|observation| {
            matches!(
                observation.kind,
                conduit_core::ObservationKind::PlanCompleted
                    | conduit_core::ObservationKind::PlanTerminal {
                        disposition: conduit_core::TerminalDisposition::Completed
                    }
            )
        });
        if !completed {
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
            terminal: TourRunTerminal::Completed,
            comparison: None,
            multi_host: None,
        })
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
    fn distinct_unimplemented_hosted_lifecycles_do_not_gain_fake_success() {
        assert_eq!(
            HostedTourExecutor::run(0, 1),
            Err(HostedTourExecutorRefusal::UnsupportedStage)
        );
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
