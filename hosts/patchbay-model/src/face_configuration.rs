//! Exact source edits initiated by compact controls on a Gear Face.

use conduit_core::ConfigurationValue;
use conduit_form::{parse_syntax_document, Argument, BackStatement};
use conduit_std_catalog::StandardConfigurationRule;

use crate::{FormEditor, FormEditorError, PatchbayGraph};

impl FormEditor {
    pub fn set_gear_configuration(
        &mut self,
        offered_revision: u64,
        offered_expanded_form_id: &conduit_core::ExpandedFormId,
        gear_name: &str,
        key: &str,
        value: ConfigurationValue,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let expanded = self.expand_form(&self.open_form)?;
        let graph = PatchbayGraph::from_expanded(&expanded)
            .map_err(|error| FormEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_form_id != offered_expanded_form_id {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let expanded_gear_id = format!("{}/{}", self.open_form, gear_name);
        let gear = expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == expanded_gear_id)
            .ok_or_else(|| FormEditorError::UnknownGear(gear_name.into()))?;
        let contract = conduit_std_catalog::supported_nucleus_contracts()
            .into_iter()
            .chain(conduit_std_catalog::standard_contracts())
            .find(|contract| contract.kind_id == gear.kind_id)
            .ok_or_else(|| FormEditorError::UnknownConfiguration(key.into()))?;
        let field = contract
            .configuration
            .iter()
            .find(|field| field.key == key)
            .ok_or_else(|| FormEditorError::UnknownConfiguration(key.into()))?;
        if !accepts(&field.rule, &value) {
            return Err(FormEditorError::InvalidConfiguration(
                configuration_refusal(&field.rule),
            ));
        }

        let document = parse_syntax_document(&self.source);
        let form = document
            .forms
            .iter()
            .find(|form| form.name.text == self.open_form)
            .ok_or_else(|| FormEditorError::UnknownForm(self.open_form.clone()))?;
        let named = form
            .back
            .iter()
            .find_map(|statement| match statement {
                BackStatement::NamedGear(named) if named.name.text == gear_name => Some(named),
                _ => None,
            })
            .ok_or_else(|| FormEditorError::UnknownGear(gear_name.into()))?;
        let spelling = configuration_spelling(&field.rule, &value);
        let mut candidate = self.source.clone();
        if let Some(argument) = named.invocation.arguments.iter().find(|argument| {
            matches!(argument,
            Argument::Named { name, .. } if name.text == key)
        }) {
            let Argument::Named { value, .. } = argument else {
                unreachable!()
            };
            candidate.replace_range(value.span.start..value.span.end, &spelling);
        } else if let Some(Argument::Positional(expression)) = contract
            .configuration
            .iter()
            .position(|candidate| candidate.key == key)
            .and_then(|index| named.invocation.arguments.get(index))
        {
            candidate.replace_range(expression.span.start..expression.span.end, &spelling);
        } else if named.invocation.arguments.is_empty() {
            candidate.insert_str(named.invocation.span.end, &format!("({key} = {spelling})"));
        } else {
            let closing = self.source[named.invocation.span.start..named.invocation.span.end]
                .rfind(')')
                .map(|offset| named.invocation.span.start + offset)
                .ok_or_else(|| {
                    FormEditorError::InvalidConfiguration(
                        "cannot locate invocation argument boundary".into(),
                    )
                })?;
            candidate.insert_str(closing, &format!(", {key} = {spelling}"));
        }
        self.apply_candidate(candidate)
    }
}

fn accepts(rule: &StandardConfigurationRule, value: &ConfigurationValue) -> bool {
    match (rule, value) {
        (StandardConfigurationRule::Any, ConfigurationValue::Bool(_)) => true,
        (
            StandardConfigurationRule::U64Range { minimum, maximum },
            ConfigurationValue::U64(value),
        )
        | (
            StandardConfigurationRule::DurationMillis { minimum, maximum },
            ConfigurationValue::U64(value),
        ) => (*minimum..=*maximum).contains(value),
        (StandardConfigurationRule::TextBytes { maximum }, ConfigurationValue::Text(value)) => {
            value.len() <= *maximum as usize
        }
        _ => false,
    }
}

fn configuration_refusal(rule: &StandardConfigurationRule) -> String {
    match rule {
        StandardConfigurationRule::Any => "expected a Boolean value".into(),
        StandardConfigurationRule::U64Range { minimum, maximum } => {
            format!("enter a number from {minimum} through {maximum}")
        }
        StandardConfigurationRule::DurationMillis { minimum, maximum } => {
            format!("enter milliseconds from {minimum} through {maximum}")
        }
        StandardConfigurationRule::TextBytes { maximum } => {
            format!("enter at most {maximum} bytes of text")
        }
    }
}

fn configuration_spelling(rule: &StandardConfigurationRule, value: &ConfigurationValue) -> String {
    match (rule, value) {
        (StandardConfigurationRule::DurationMillis { .. }, ConfigurationValue::U64(value)) => {
            format!("{value}ms")
        }
        (_, ConfigurationValue::Bool(value)) => value.to_string(),
        (_, ConfigurationValue::U64(value)) => value.to_string(),
        (_, ConfigurationValue::Text(value)) => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
    }
}
