//! Exact source edits initiated by compact controls on a Gear Face.

use conduit_core::{
    ConfigurationValue, HumanInteractionProposal, InfoBool, InteractionFamily,
    InteractionProposalPayload, InteractionValue, KindId, Quantity, BOOL_INFO_ID, QUANTITY_INFO_ID,
    TEXT_INFO_ID,
};
use conduit_form::{parse_syntax_document, Argument, BackStatement};
use conduit_std_catalog::StandardConfigurationRule;

use crate::{FormEditor, FormEditorError, PatchbayGraph};

impl FormEditor {
    /// Applies one common typed interaction proposal to checked configuration.
    /// Configuration remains a replan operation: this reseals the checked and
    /// expanded identities rather than mutating an active implementation.
    pub fn apply_gear_configuration_proposal(
        &mut self,
        offered_revision: u64,
        offered_expanded_form_id: &conduit_core::ExpandedFormId,
        gear_name: &str,
        key: &str,
        proposal: &HumanInteractionProposal,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let graph = self.patchbay_graph_for_authoring(&self.open_form)?;
        if &graph.expanded_form_id != offered_expanded_form_id {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let gear_id = format!("{}/{}", self.open_form, gear_name);
        let gear = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == gear_id)
            .ok_or_else(|| FormEditorError::UnknownConfiguration(key.into()))?;
        let control = gear
            .controls
            .iter()
            .find(|control| control.key == key)
            .ok_or_else(|| FormEditorError::UnknownConfiguration(key.into()))?;
        let interaction = control.interaction.as_ref().ok_or_else(|| {
            FormEditorError::InvalidConfiguration(
                "configuration is not representable by the common interaction contract".into(),
            )
        })?;
        proposal
            .validate_against(&interaction.contract, &interaction.state)
            .map_err(|refusal| {
                FormEditorError::InvalidConfiguration(format!(
                    "common interaction refused: {refusal:?}"
                ))
            })?;
        let value = configuration_from_proposal(&interaction.contract.family, proposal)?;
        self.set_gear_configuration_exact(
            offered_revision,
            offered_expanded_form_id,
            gear_name,
            key,
            value,
        )
    }

    pub fn set_gear_configuration(
        &mut self,
        offered_revision: u64,
        offered_expanded_form_id: &conduit_core::ExpandedFormId,
        gear_name: &str,
        key: &str,
        value: ConfigurationValue,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let graph = self.patchbay_graph_for_authoring(&self.open_form)?;
        if &graph.expanded_form_id != offered_expanded_form_id {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let gear_id = format!("{}/{}", self.open_form, gear_name);
        let gear = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == gear_id)
            .ok_or_else(|| FormEditorError::UnknownConfiguration(key.into()))?;
        let control = gear
            .controls
            .iter()
            .find(|control| control.key == key)
            .ok_or_else(|| FormEditorError::UnknownConfiguration(key.into()))?;
        let rule = conduit_std_catalog::supported_nucleus_contracts()
            .into_iter()
            .chain(conduit_std_catalog::standard_contracts())
            .find(|contract| contract.kind_id == gear.kind_id)
            .and_then(|contract| {
                contract
                    .configuration
                    .into_iter()
                    .find(|field| field.key == key)
            })
            .ok_or_else(|| FormEditorError::UnknownConfiguration(key.into()))?
            .rule;
        if !accepts(&rule, &value) {
            return Err(FormEditorError::InvalidConfiguration(
                configuration_refusal(&rule),
            ));
        }
        let interaction = control.interaction.as_ref().ok_or_else(|| {
            FormEditorError::InvalidConfiguration(
                "configuration is not representable by the common interaction contract".into(),
            )
        })?;
        let proposal = proposal_for_configuration(interaction, value)?;
        self.apply_gear_configuration_proposal(
            offered_revision,
            offered_expanded_form_id,
            gear_name,
            key,
            &proposal,
        )
    }

    fn set_gear_configuration_exact(
        &mut self,
        offered_revision: u64,
        offered_expanded_form_id: &conduit_core::ExpandedFormId,
        gear_name: &str,
        key: &str,
        value: ConfigurationValue,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let authoring = self.expand_form_for_authoring(&self.open_form)?;
        let graph = PatchbayGraph::from_authoring(&authoring)
            .map_err(|error| FormEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_form_id != offered_expanded_form_id {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let expanded_gear_id = format!("{}/{}", self.open_form, gear_name);
        let gear = authoring
            .expanded
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

fn proposal_for_configuration(
    interaction: &crate::FaceInteraction,
    value: ConfigurationValue,
) -> Result<HumanInteractionProposal, FormEditorError> {
    let typed = match (&interaction.contract.family, value) {
        (InteractionFamily::Boolean, ConfigurationValue::Bool(value)) => InteractionValue::new(
            KindId::from(BOOL_INFO_ID),
            if value {
                InfoBool::TRUE
            } else {
                InfoBool::FALSE
            }
            .encode()
            .to_vec(),
        ),
        (InteractionFamily::Scalar { unit, .. }, ConfigurationValue::U64(value)) => {
            let value = i64::try_from(value).map_err(|_| {
                FormEditorError::InvalidConfiguration("scalar exceeds interaction range".into())
            })?;
            InteractionValue::new(
                KindId::from(QUANTITY_INFO_ID),
                Quantity::new(value, *unit).encode().to_vec(),
            )
        }
        (InteractionFamily::Scalar { unit, .. }, ConfigurationValue::I64(value)) => {
            InteractionValue::new(
                KindId::from(QUANTITY_INFO_ID),
                Quantity::new(value, *unit).encode().to_vec(),
            )
        }
        (InteractionFamily::ChooseOne { value_kind, .. }, ConfigurationValue::Text(value)) => {
            InteractionValue::new(value_kind.clone(), value.into_bytes())
        }
        (InteractionFamily::Text { .. }, ConfigurationValue::Text(value)) => {
            InteractionValue::new(KindId::from(TEXT_INFO_ID), value.into_bytes())
        }
        _ => {
            return Err(FormEditorError::InvalidConfiguration(
                "value does not fit the common interaction family".into(),
            ))
        }
    }
    .map_err(|refusal| {
        FormEditorError::InvalidConfiguration(format!(
            "common interaction value refused: {refusal:?}"
        ))
    })?;
    HumanInteractionProposal::new(
        &interaction.contract,
        &interaction.state,
        interaction.state.revision.saturating_add(1),
        InteractionProposalPayload::Values(vec![typed]),
    )
    .map_err(|refusal| {
        FormEditorError::InvalidConfiguration(format!(
            "common interaction proposal refused: {refusal:?}"
        ))
    })
}

fn configuration_from_proposal(
    family: &InteractionFamily,
    proposal: &HumanInteractionProposal,
) -> Result<ConfigurationValue, FormEditorError> {
    let InteractionProposalPayload::Values(values) = &proposal.payload else {
        return Err(FormEditorError::InvalidConfiguration(
            "configuration requires an absolute typed value".into(),
        ));
    };
    let [value] = values.as_slice() else {
        return Err(FormEditorError::InvalidConfiguration(
            "configuration requires exactly one typed value".into(),
        ));
    };
    match family {
        InteractionFamily::Boolean if value.value_kind.as_str() == BOOL_INFO_ID => {
            InfoBool::decode(&value.canonical_bytes)
                .map(|decoded| ConfigurationValue::Bool(decoded == InfoBool::TRUE))
                .map_err(|_| FormEditorError::InvalidConfiguration("malformed Boolean".into()))
        }
        InteractionFamily::Scalar { unit, .. } if value.value_kind.as_str() == QUANTITY_INFO_ID => {
            let decoded = Quantity::decode(&value.canonical_bytes).map_err(|_| {
                FormEditorError::InvalidConfiguration("malformed scalar quantity".into())
            })?;
            if *unit == conduit_core::QuantityUnit::Millionth {
                Ok(ConfigurationValue::I64(decoded.value()))
            } else {
                decoded
                    .value()
                    .try_into()
                    .map(ConfigurationValue::U64)
                    .map_err(|_| {
                        FormEditorError::InvalidConfiguration(
                            "negative value for unsigned configuration".into(),
                        )
                    })
            }
        }
        InteractionFamily::ChooseOne { .. } | InteractionFamily::Text { .. } => {
            core::str::from_utf8(&value.canonical_bytes)
                .map(|text| ConfigurationValue::Text(text.into()))
                .map_err(|_| FormEditorError::InvalidConfiguration("malformed text".into()))
        }
        _ if value.value_kind.as_str() == TEXT_INFO_ID => {
            core::str::from_utf8(&value.canonical_bytes)
                .map(|text| ConfigurationValue::Text(text.into()))
                .map_err(|_| FormEditorError::InvalidConfiguration("malformed text".into()))
        }
        _ => Err(FormEditorError::InvalidConfiguration(
            "unsupported configuration interaction family".into(),
        )),
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
        (
            StandardConfigurationRule::I64Range { minimum, maximum },
            ConfigurationValue::I64(value),
        ) => (*minimum..=*maximum).contains(value),
        (StandardConfigurationRule::TextBytes { maximum }, ConfigurationValue::Text(value)) => {
            value.len() <= *maximum as usize
        }
        (StandardConfigurationRule::TextOneOf { values }, ConfigurationValue::Text(value)) => {
            values.contains(value)
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
        StandardConfigurationRule::I64Range { minimum, maximum } => {
            format!("enter scalar microunits from {minimum} through {maximum}")
        }
        StandardConfigurationRule::DurationMillis { minimum, maximum } => {
            format!("enter milliseconds from {minimum} through {maximum}")
        }
        StandardConfigurationRule::TextBytes { maximum } => {
            format!("enter at most {maximum} bytes of text")
        }
        StandardConfigurationRule::TextOneOf { values } => {
            format!("choose one of {}", values.join(", "))
        }
    }
}

pub(crate) fn configuration_spelling(
    rule: &StandardConfigurationRule,
    value: &ConfigurationValue,
) -> String {
    match (rule, value) {
        (StandardConfigurationRule::DurationMillis { .. }, ConfigurationValue::U64(value)) => {
            format!("{value}ms")
        }
        (_, ConfigurationValue::Bool(value)) => value.to_string(),
        (_, ConfigurationValue::U64(value)) => value.to_string(),
        (_, ConfigurationValue::I64(value)) => value.to_string(),
        (_, ConfigurationValue::Text(value)) => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
        (_, ConfigurationValue::Structured(value)) => format!(
            "<structured:{}:{}-bytes>",
            value.profile().as_str(),
            value.canonical_value().len()
        ),
    }
}
