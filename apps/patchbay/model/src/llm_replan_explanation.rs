//! Renderer-neutral Patchbay explanation of unchanged LLM meaning across replanning.

use conduit_ai::{LlmPlanningRefusal, ReplacementLlmRun};
use serde::{Deserialize, Serialize};

pub const MAX_LLM_REPLAN_EXPLANATION_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossHostLlmReplanExplanation {
    pub source_document_id: String,
    pub checked_form_id: String,
    pub expanded_form_id: String,
    pub interrupted_plan_id: String,
    pub replacement_plan_id: String,
    pub interrupted_play_id: String,
    pub replacement_play_id: String,
    pub unchanged_form: bool,
    pub automatic_migration: bool,
    pub stale_completion_policy: &'static str,
    pub summary: String,
}

pub fn explain_cross_host_llm_replan(
    replacement: &ReplacementLlmRun,
) -> Result<CrossHostLlmReplanExplanation, String> {
    let old = &replacement.interrupted.run;
    let current = &replacement.current;
    let summary = format!(
        "Form source={} checked={} expanded={} is unchanged. Plan {} / Play {} stopped after {:?}. Fresh realization truth sealed distinct Plan {} / Play {}; no stateful migration or automatic retry occurred, and completions from the interrupted Play are stale.",
        current.source_document_id.as_str(),
        current.checked_form_id.as_str(),
        current.expanded_form_id.as_str(),
        old.plan_id.as_str(),
        old.active_play_id.as_str(),
        replacement.interrupted.reason,
        current.plan_id.as_str(),
        current.active_play_id.as_str(),
    );
    if summary.len() > MAX_LLM_REPLAN_EXPLANATION_BYTES {
        return Err("LLM replan explanation exceeds its finite bound".into());
    }
    Ok(CrossHostLlmReplanExplanation {
        source_document_id: current.source_document_id.as_str().into(),
        checked_form_id: current.checked_form_id.as_str().into(),
        expanded_form_id: current.expanded_form_id.as_str().into(),
        interrupted_plan_id: old.plan_id.as_str().into(),
        replacement_plan_id: current.plan_id.as_str().into(),
        interrupted_play_id: old.active_play_id.as_str().into(),
        replacement_play_id: current.active_play_id.as_str().into(),
        unchanged_form: true,
        automatic_migration: false,
        stale_completion_policy: "reject",
        summary,
    })
}

pub fn explain_missing_llm_realization(refusal: LlmPlanningRefusal) -> &'static str {
    match refusal {
        LlmPlanningRefusal::MissingLlmRealization => {
            "No current Host offers a compatible LLM realization; the unchanged Form remains unsatisfied."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_ai::{
        CrossHostLlmRun, InterruptedLlmRun, LlmInterruptionReason, LlmRealizationPart,
    };
    use conduit_core::{
        ActivePlayId, BootId, CheckedFormId, ExpandedFormId, HostId, ImplementationId,
        OfferGeneration, PlanId, SourceDocumentId,
    };

    fn run(plan: &str, play: &str, host: &str, generation: u64) -> CrossHostLlmRun {
        CrossHostLlmRun {
            source_document_id: SourceDocumentId::from("source/unchanged"),
            checked_form_id: CheckedFormId::from("checked/unchanged"),
            expanded_form_id: ExpandedFormId::from("expanded/unchanged"),
            plan_id: PlanId::from(plan),
            active_play_id: ActivePlayId::from(play),
            request_id: format!("request/{play}"),
            parts: vec![LlmRealizationPart {
                host_id: HostId::from(host),
                boot_id: BootId::from(format!("boot/{host}")),
                offer_generation: OfferGeneration(generation),
                implementations: vec![ImplementationId::from("provider/http")],
            }],
            remote_line_count: 2,
        }
    }

    #[test]
    fn explains_unchanged_form_distinct_realization_and_specific_refusal() {
        let replacement = ReplacementLlmRun {
            interrupted: InterruptedLlmRun {
                run: run("plan/a", "play/a", "provider/a", 1),
                reason: LlmInterruptionReason::ModelProviderLost,
            },
            current: run("plan/b", "play/b", "provider/b", 2),
        };
        let explanation = explain_cross_host_llm_replan(&replacement).unwrap();
        assert!(explanation.unchanged_form);
        assert!(!explanation.automatic_migration);
        assert_eq!(explanation.stale_completion_policy, "reject");
        assert!(explanation.summary.contains("Fresh realization truth"));
        assert!(
            explain_missing_llm_realization(LlmPlanningRefusal::MissingLlmRealization)
                .contains("unchanged Form remains unsatisfied")
        );
    }
}
