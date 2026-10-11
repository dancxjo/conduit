//! Exact source edits initiated by compact controls on a gear Front.

use conduit_core::{
    ConfigurationValue, InfoBool, KindId, Quantity, BOOL_INFO_ID, QUANTITY_INFO_ID,
};
use conduit_human::{
    HumanInteractionProposal, InteractionFamily, InteractionProposalPayload, InteractionValue,
    TEXT_INFO_ID,
};
use conduit_plot::{parse_syntax_document, Argument, BackStatement};
use conduit_semantic_catalog::KindConfigurationRule;

use crate::{PatchbayGraph, PlotEditor, PlotEditorError};

impl PlotEditor {
    /// Applies one common typed interaction proposal to checked configuration.
    /// Configuration remains a replan operation: this reseals the checked and
    /// expanded identities rather than mutating an active implementation.
    pub fn apply_gear_configuration_proposal(
        &mut self,
        offered_revision: u64,
        offered_expanded_plot_id: &conduit_core::ExpandedPlotId,
        gear_name: &str,
        key: &str,
        proposal: &HumanInteractionProposal,
    ) -> Result<(), PlotEditorError> {
        self.require_revision(offered_revision)?;
        let graph = self.patchbay_graph_for_authoring(&self.open_plot)?;
        if &graph.expanded_plot_id != offered_expanded_plot_id {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        let gear_id = format!("{}/{}", self.open_plot, gear_name);
        let gear = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == gear_id)
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        let control = gear
            .controls
            .iter()
            .find(|control| control.key == key)
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        let interaction = control.interaction.as_ref().ok_or_else(|| {
            PlotEditorError::InvalidConfiguration(
                "configuration is not representable by the common interaction contract".into(),
            )
        })?;
        let field = conduit_semantic_catalog::supported_nucleus_contracts()
            .into_iter()
            .find(|contract| contract.kind_id == gear.kind_id)
            .and_then(|contract| {
                contract
                    .configuration
                    .into_iter()
                    .find(|field| field.key == key)
            })
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        proposal
            .validate_against(&interaction.contract, &interaction.state)
            .map_err(|refusal| {
                PlotEditorError::InvalidConfiguration(format!(
                    "common interaction refused: {refusal:?}"
                ))
            })?;
        let value =
            configuration_from_proposal(&interaction.contract.family, &field.rule, proposal)?;
        self.set_gear_configuration_exact(
            offered_revision,
            offered_expanded_plot_id,
            gear_name,
            key,
            value,
        )
    }

    pub fn set_gear_configuration(
        &mut self,
        offered_revision: u64,
        offered_expanded_plot_id: &conduit_core::ExpandedPlotId,
        gear_name: &str,
        key: &str,
        value: ConfigurationValue,
    ) -> Result<(), PlotEditorError> {
        self.require_revision(offered_revision)?;
        let graph = self.patchbay_graph_for_authoring(&self.open_plot)?;
        if &graph.expanded_plot_id != offered_expanded_plot_id {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        let gear_id = format!("{}/{}", self.open_plot, gear_name);
        let gear = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == gear_id)
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        let control = gear
            .controls
            .iter()
            .find(|control| control.key == key)
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        let field = conduit_semantic_catalog::supported_nucleus_contracts()
            .into_iter()
            .find(|contract| contract.kind_id == gear.kind_id)
            .and_then(|contract| {
                contract
                    .configuration
                    .into_iter()
                    .find(|field| field.key == key)
            })
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        conduit_plot::validate_configuration_value(&field, &value)
            .map_err(|error| PlotEditorError::InvalidConfiguration(error.to_string()))?;
        let interaction = control.interaction.as_ref().ok_or_else(|| {
            PlotEditorError::InvalidConfiguration(
                "configuration is not representable by the common interaction contract".into(),
            )
        })?;
        let proposal = proposal_for_configuration(interaction, value)?;
        self.apply_gear_configuration_proposal(
            offered_revision,
            offered_expanded_plot_id,
            gear_name,
            key,
            &proposal,
        )
    }

    fn set_gear_configuration_exact(
        &mut self,
        offered_revision: u64,
        offered_expanded_plot_id: &conduit_core::ExpandedPlotId,
        gear_name: &str,
        key: &str,
        value: ConfigurationValue,
    ) -> Result<(), PlotEditorError> {
        self.require_revision(offered_revision)?;
        let authoring = self.expand_plot_for_authoring(&self.open_plot)?;
        let graph = PatchbayGraph::from_authoring(&authoring)
            .map_err(|error| PlotEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_plot_id != offered_expanded_plot_id {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        let expanded_gear_id = format!("{}/{}", self.open_plot, gear_name);
        let gear = authoring
            .expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == expanded_gear_id)
            .ok_or_else(|| PlotEditorError::UnknownGear(gear_name.into()))?;
        let contract = conduit_semantic_catalog::supported_nucleus_contracts()
            .into_iter()
            .find(|contract| contract.kind_id == gear.kind_id)
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        let field = contract
            .configuration
            .iter()
            .find(|field| field.key == key)
            .ok_or_else(|| PlotEditorError::UnknownConfiguration(key.into()))?;
        conduit_plot::validate_configuration_value(field, &value)
            .map_err(|error| PlotEditorError::InvalidConfiguration(error.to_string()))?;

        let document = parse_syntax_document(&self.source);
        let plot = document
            .plots
            .iter()
            .find(|plot| plot.name.text == self.open_plot)
            .ok_or_else(|| PlotEditorError::UnknownPlot(self.open_plot.clone()))?;
        let named = plot
            .back
            .iter()
            .find_map(|statement| match statement {
                BackStatement::NamedGear(named) if named.name.text == gear_name => Some(named),
                _ => None,
            })
            .ok_or_else(|| PlotEditorError::UnknownGear(gear_name.into()))?;
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
                    PlotEditorError::InvalidConfiguration(
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
) -> Result<HumanInteractionProposal, PlotEditorError> {
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
        (InteractionFamily::Scalar(family), ConfigurationValue::U64(value)) => {
            let value = i64::try_from(value).map_err(|_| {
                PlotEditorError::InvalidConfiguration("scalar exceeds interaction range".into())
            })?;
            InteractionValue::new(
                KindId::from(QUANTITY_INFO_ID),
                Quantity::new(value, *family.unit()).encode().to_vec(),
            )
        }
        (InteractionFamily::Scalar(family), ConfigurationValue::I64(value)) => {
            InteractionValue::new(
                KindId::from(QUANTITY_INFO_ID),
                Quantity::new(value, *family.unit()).encode().to_vec(),
            )
        }
        (InteractionFamily::Scalar(_), ConfigurationValue::Quantity(value)) => {
            InteractionValue::new(
                KindId::from(QUANTITY_INFO_ID),
                value.value().encode().to_vec(),
            )
        }
        (InteractionFamily::ChooseOne(family), ConfigurationValue::Text(value)) => {
            InteractionValue::new(
                KindId::from(family.value_kind().get().as_str()),
                value.into_bytes(),
            )
        }
        (InteractionFamily::Text(_), ConfigurationValue::Text(value)) => {
            InteractionValue::new(KindId::from(TEXT_INFO_ID), value.into_bytes())
        }
        (
            InteractionFamily::Structured(_),
            value @ (ConfigurationValue::Unit(_)
            | ConfigurationValue::Quantity(_)
            | ConfigurationValue::TemperatureDifference(_)),
        ) => patchbay_graph::physical_interaction_value(&value)
            .map_err(|_| conduit_human::InteractionRefusal::MalformedValue),
        _ => {
            return Err(PlotEditorError::InvalidConfiguration(
                "value does not fit the common interaction family".into(),
            ));
        }
    }
    .map_err(|refusal| {
        PlotEditorError::InvalidConfiguration(format!(
            "common interaction value refused: {refusal:?}"
        ))
    })?;
    HumanInteractionProposal::new(
        &interaction.contract,
        &interaction.state,
        interaction.state.revision.saturating_add(1),
        InteractionProposalPayload::selected(vec![typed]).map_err(|refusal| {
            PlotEditorError::InvalidConfiguration(format!(
                "common interaction payload refused: {refusal:?}"
            ))
        })?,
    )
    .map_err(|refusal| {
        PlotEditorError::InvalidConfiguration(format!(
            "common interaction proposal refused: {refusal:?}"
        ))
    })
}

fn configuration_from_proposal(
    family: &InteractionFamily,
    rule: &KindConfigurationRule,
    proposal: &HumanInteractionProposal,
) -> Result<ConfigurationValue, PlotEditorError> {
    let InteractionProposalPayload::Values(values) = &proposal.payload else {
        return Err(PlotEditorError::InvalidConfiguration(
            "configuration requires an absolute typed value".into(),
        ));
    };
    let [value] = values.get().as_slice() else {
        return Err(PlotEditorError::InvalidConfiguration(
            "configuration requires exactly one typed value".into(),
        ));
    };
    match family {
        InteractionFamily::Boolean if value.kind() == BOOL_INFO_ID => {
            InfoBool::decode(value.bytes())
                .map(|decoded| ConfigurationValue::Bool(decoded == InfoBool::TRUE))
                .map_err(|_| PlotEditorError::InvalidConfiguration("malformed Boolean".into()))
        }
        InteractionFamily::Scalar(family) if value.kind() == QUANTITY_INFO_ID => {
            let decoded = Quantity::decode(value.bytes()).map_err(|_| {
                PlotEditorError::InvalidConfiguration("malformed scalar quantity".into())
            })?;
            if *family.unit() == conduit_core::Unit::Millionth {
                decoded
                    .to_i64(*family.unit())
                    .map(ConfigurationValue::I64)
                    .map_err(|_| {
                        PlotEditorError::InvalidConfiguration("inexact scalar quantity".into())
                    })
            } else if matches!(rule, KindConfigurationRule::DurationMillis { .. }) {
                decoded
                    .convert_to_u64(*family.unit())
                    .map(ConfigurationValue::U64)
                    .map_err(|_| {
                        PlotEditorError::InvalidConfiguration(
                            "inexact, incompatible, or negative quantity".into(),
                        )
                    })
            } else if *family.unit() != conduit_core::Unit::One {
                decoded
                    .convert(*family.unit())
                    .and_then(|quantity| {
                        conduit_core::QuantityConfigurationValue::from_value(quantity)
                            .map_err(|_| conduit_core::QuantityConversionRefusal::Overflow)
                    })
                    .map(ConfigurationValue::Quantity)
                    .map_err(|_| {
                        PlotEditorError::InvalidConfiguration(
                            "inexact or incompatible quantity".into(),
                        )
                    })
            } else {
                decoded
                    .convert_to_u64(*family.unit())
                    .map(ConfigurationValue::U64)
                    .map_err(|_| {
                        PlotEditorError::InvalidConfiguration(
                            "negative value for unsigned configuration".into(),
                        )
                    })
            }
        }
        InteractionFamily::Structured(_) => {
            let invalid = || {
                PlotEditorError::InvalidConfiguration("malformed typed physical interaction".into())
            };
            let wrapped = conduit_core::StructuredInfoValue::from_canonical_bytes(value.bytes())
                .map_err(|_| invalid())?;
            let conduit_core::StructuredInfoValueShape::Leaf(bytes) = wrapped.shape() else {
                return Err(invalid());
            };
            match rule {
                KindConfigurationRule::Unit => {
                    let unit = conduit_core::Unit::decode(bytes).map_err(|_| invalid())?;
                    conduit_core::UnitConfigurationValue::new(unit, unit.canonical_symbol())
                        .map(ConfigurationValue::Unit)
                        .ok_or_else(invalid)
                }
                KindConfigurationRule::Quantity => {
                    let quantity = Quantity::decode(bytes).map_err(|_| invalid())?;
                    conduit_core::QuantityConfigurationValue::from_value(quantity)
                        .map(ConfigurationValue::Quantity)
                        .map_err(|_| invalid())
                }
                KindConfigurationRule::TemperatureDifference => {
                    let quantity = Quantity::decode(bytes).map_err(|_| invalid())?;
                    let difference =
                        conduit_core::ExactTemperatureDifference::from_quantity(quantity)
                            .map_err(|_| invalid())?;
                    conduit_core::ExactTemperatureDifferenceConfigurationValue::new(
                        difference,
                        quantity.canonical_literal().map_err(|_| invalid())?,
                    )
                    .map(ConfigurationValue::TemperatureDifference)
                    .ok_or_else(invalid)
                }
                _ => Err(invalid()),
            }
        }
        InteractionFamily::ChooseOne(_) | InteractionFamily::Text(_) => {
            core::str::from_utf8(value.bytes())
                .map(|text| ConfigurationValue::Text(text.into()))
                .map_err(|_| PlotEditorError::InvalidConfiguration("malformed text".into()))
        }
        _ if value.kind() == TEXT_INFO_ID => core::str::from_utf8(value.bytes())
            .map(|text| ConfigurationValue::Text(text.into()))
            .map_err(|_| PlotEditorError::InvalidConfiguration("malformed text".into())),
        _ => Err(PlotEditorError::InvalidConfiguration(
            "unsupported configuration interaction family".into(),
        )),
    }
}

pub(crate) fn configuration_spelling(
    rule: &KindConfigurationRule,
    value: &ConfigurationValue,
) -> String {
    match (rule, value) {
        (KindConfigurationRule::DurationMillis { .. }, ConfigurationValue::U64(value)) => {
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
        (_, ConfigurationValue::Quantity(value)) => value.source().to_string(),
        (_, ConfigurationValue::Unit(value)) => value.source().to_string(),
        (_, ConfigurationValue::TemperatureDifference(value)) => value.source().to_string(),
    }
}
