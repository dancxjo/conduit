//! Exact finite-call limits. Provider counters are reported work, not a CPU
//! deadline: a decoder token cap cannot bound opaque provider prefill work.
use super::*;
use conduit_core::ConfigurationValue;

pub(super) struct FiniteWorkBudget {
    bounds: LlmWorkBounds,
}
impl FiniteWorkBudget {
    pub(super) fn admit(
        placement: &PlannedGear,
        offered: LlmWorkBounds,
        input_bytes: usize,
    ) -> Result<Self, LocalModelAdapterTerminal> {
        use LocalModelAdapterTerminal::Refused;
        const KEYS: [&str; 5] = [
            "maximum-input-bytes",
            "maximum-context-items",
            "maximum-output-bytes",
            "maximum-work-units",
            "maximum-history-items",
        ];
        if placement.configuration.len() > KEYS.len()
            || placement
                .configuration
                .iter()
                .any(|entry| !KEYS.contains(&entry.key.as_str()))
        {
            return Err(Refused);
        }
        let count = |key: &str, fallback: u64| {
            let mut matches = placement
                .configuration
                .iter()
                .filter(|entry| entry.key == key);
            let value = match matches.next() {
                None => fallback,
                Some(entry) => match entry.value {
                    ConfigurationValue::U64(value) => value,
                    _ => return Err(Refused),
                },
            };
            if matches.next().is_some() {
                return Err(Refused);
            }
            Ok(value.min(fallback))
        };
        let bounds = LlmWorkBounds::new(
            count(KEYS[0], offered.maximum_input_bytes())?,
            count(KEYS[1], offered.maximum_context_items())?,
            count(KEYS[2], offered.maximum_output_bytes())?,
            count(KEYS[3], offered.maximum_work_units())?,
            count(KEYS[4], offered.maximum_history_items())?,
        )
        .map_err(|_| Refused)?;
        // This finite adapter consumes one prompt and retains no conversation
        // history. Check these known coordinates before provider execution.
        if input_bytes as u64 > bounds.maximum_input_bytes() || bounds.maximum_context_items() < 1 {
            return Err(Refused);
        }
        Ok(Self { bounds })
    }
    pub(super) fn output_bytes(&self) -> u64 {
        self.bounds.maximum_output_bytes()
    }
    pub(super) fn decoder_tokens(&self, proposed: u64) -> u64 {
        proposed.min(self.bounds.maximum_work_units())
    }
    fn work(&self, reported: Option<u64>) -> Result<u64, LocalModelAdapterTerminal> {
        let work = reported.ok_or(LocalModelAdapterTerminal::InvalidStructuredResult)?;
        if work > self.bounds.maximum_work_units() {
            return Err(LocalModelAdapterTerminal::Failed);
        }
        Ok(work)
    }
    fn generation_work(
        &self,
        prompt: Option<u64>,
        generated: Option<u64>,
    ) -> Result<u64, LocalModelAdapterTerminal> {
        self.work(prompt.zip(generated).and_then(|(p, g)| p.checked_add(g)))
    }
}
impl OllamaLocalModelAdapter {
    pub(super) fn generate_with_budget(
        &self,
        input: &str,
        tokens: u64,
        structured: bool,
        budget: &FiniteWorkBudget,
    ) -> Result<(GenerateResponse, u64), LocalModelAdapterTerminal> {
        let response = self.generate(input, budget.decoder_tokens(tokens), structured)?;
        let work = budget.generation_work(response.prompt_eval_count, response.eval_count)?;
        Ok((response, work))
    }
    pub(super) fn embed_with_budget(
        &self,
        input: &str,
        budget: &FiniteWorkBudget,
    ) -> Result<(EmbedResponse, u64), LocalModelAdapterTerminal> {
        let response = self.embed(input)?;
        let work = budget.work(response.prompt_eval_count)?;
        Ok((response, work))
    }
    pub(super) fn chat_with_budget(
        &self,
        system: &str,
        data: &str,
        tokens: u64,
        budget: &FiniteWorkBudget,
    ) -> Result<(ChatResponse, u64), LocalModelAdapterTerminal> {
        let response = self.chat_present(system, data, budget.decoder_tokens(tokens))?;
        let work = budget.generation_work(response.prompt_eval_count, response.eval_count)?;
        Ok((response, work))
    }
}

#[cfg(test)]
#[path = "work_budget_tests.rs"]
mod tests;
