use crate::prelude::*;
use crate::{
    select_realization_with_scoped_policy, HardRealizationRequirements, PlannerError,
    PlannerFactRef, PlannerFactValue, PlannerPreference, PolicyLayer, PolicyScope, PolicySourceId,
    PolicySourceRevision, RealizationPreference, ReviewedObservation, ScopedRealizationSelection,
    MAXIMUM_PLANNER_POLICY_CLAUSES,
};
use conduit_core::{
    stable_realization_boolean, stable_realization_category, CharacteristicId, CharacteristicValue,
    HostAdvertisement, RealizationAdvertisement, RealizationCharacteristic,
};
use conduit_form::CheckedGear;

pub const DOS_SHELL_STYLE_ID: &str = "conduit.style/dos-shell@1";
pub const PRESENTATION_TEXT_LAYOUT: &str = "presentation/text-layout";
pub const PRESENTATION_DENSITY: &str = "presentation/density";
pub const PRESENTATION_FRAMING: &str = "presentation/framing";
pub const PRESENTATION_PALETTE_CLASS: &str = "presentation/palette-class";
pub const PRESENTATION_KEYBOARD_VISIBLE: &str = "presentation/keyboard-visible";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StyleId(String);

impl StyleId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StyleId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// A named finite bundle of soft presentation preferences. It lowers into the
/// ordinary C2/C3 policy path and carries no renderer-owned style object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedStyle {
    pub style_id: StyleId,
    pub revision: u64,
    pub preferences: Vec<PlannerPreference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylePreferenceOutcome {
    Matched,
    Unmatched,
    Unavailable,
    Ranked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StylePreferenceEvidence {
    pub clause_index: u16,
    pub fact: PlannerFactRef,
    pub outcome: StylePreferenceOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleSelection {
    pub scoped: ScopedRealizationSelection,
    pub style_id: StyleId,
    pub style_revision: u64,
    pub preferences: Vec<StylePreferenceEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationStyleFacts {
    pub text_layout: Option<String>,
    pub density: Option<String>,
    pub framing: Option<String>,
    pub palette_class: Option<String>,
    pub keyboard_visible: Option<bool>,
}

pub fn dos_shell_style() -> NamedStyle {
    NamedStyle {
        style_id: StyleId::from(DOS_SHELL_STYLE_ID),
        revision: 1,
        preferences: vec![
            prefer(
                PRESENTATION_TEXT_LAYOUT,
                PlannerFactValue::Category("fixed-cell".into()),
            ),
            prefer(
                PRESENTATION_DENSITY,
                PlannerFactValue::Category("compact".into()),
            ),
            prefer(
                PRESENTATION_FRAMING,
                PlannerFactValue::Category("hard-line".into()),
            ),
            prefer(
                PRESENTATION_KEYBOARD_VISIBLE,
                PlannerFactValue::Boolean(true),
            ),
            prefer(
                PRESENTATION_PALETTE_CLASS,
                PlannerFactValue::Category("phosphor-cyan-amber".into()),
            ),
        ],
    }
}

/// Builds stable facts an implementation can truthfully advertise. Missing
/// dimensions remain unknown; they are never filled from STYLE defaults.
pub fn presentation_style_characteristics(
    facts: &PresentationStyleFacts,
) -> Vec<RealizationCharacteristic> {
    let mut result = Vec::new();
    if let Some(value) = &facts.text_layout {
        result.push(category(
            PRESENTATION_TEXT_LAYOUT,
            "Text layout",
            "Stable presentation text layout class; not a font family",
            &["fixed-cell", "flow", "spoken-linear"],
            value,
        ));
    }
    if let Some(value) = &facts.density {
        result.push(category(
            PRESENTATION_DENSITY,
            "Density",
            "Stable presentation information density class",
            &["compact", "comfortable", "expanded"],
            value,
        ));
    }
    if let Some(value) = &facts.framing {
        result.push(category(
            PRESENTATION_FRAMING,
            "Framing",
            "Stable structural framing class; not toolkit borders",
            &["hard-line", "minimal", "spoken-boundary"],
            value,
        ));
    }
    if let Some(value) = &facts.palette_class {
        result.push(category(
            PRESENTATION_PALETTE_CLASS,
            "Palette class",
            "Stable abstract palette class; never raw RGB or CSS",
            &["monochrome", "phosphor-cyan-amber", "system-adaptive"],
            value,
        ));
    }
    if let Some(value) = facts.keyboard_visible {
        result.push(stable_realization_boolean(
            PRESENTATION_KEYBOARD_VISIBLE,
            "Keyboard visibility",
            "Whether keyboard interaction remains visibly discoverable",
            value,
        ));
    }
    result
}

#[allow(clippy::too_many_arguments)]
pub fn select_realization_with_style(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    requirements: &HardRealizationRequirements,
    requirement_source: PolicySourceRevision,
    policy_layers: &[PolicyLayer],
    style: &NamedStyle,
    observations: &[ReviewedObservation],
    observation_epoch: u64,
) -> Result<StyleSelection, PlannerError> {
    let style_layer = style.lower()?;
    let mut layers = policy_layers.to_vec();
    layers.push(style_layer);
    let scoped = select_realization_with_scoped_policy(
        gear,
        hosts,
        advertisements,
        requirements,
        requirement_source,
        &layers,
        observations,
        observation_epoch,
    )?;
    let selected = selected_advertisement(&scoped, advertisements);
    let mut evidence = Vec::with_capacity(style.preferences.len());
    for (index, preference) in style.preferences.iter().enumerate() {
        evidence.push(StylePreferenceEvidence {
            clause_index: u16::try_from(index).map_err(|_| {
                PlannerError::PlannerLimitExceeded("STYLE clause index exceeds u16".into())
            })?,
            fact: preference.fact().clone(),
            outcome: preference_outcome(selected, preference),
        });
    }
    Ok(StyleSelection {
        scoped,
        style_id: style.style_id.clone(),
        style_revision: style.revision,
        preferences: evidence,
    })
}

impl NamedStyle {
    pub fn lower(&self) -> Result<PolicyLayer, PlannerError> {
        if self.style_id.as_str().is_empty() || self.revision == 0 {
            return invalid("STYLE identity must be non-empty and revision non-zero");
        }
        if self.preferences.len() > MAXIMUM_PLANNER_POLICY_CLAUSES {
            return Err(PlannerError::PlannerLimitExceeded(format!(
                "STYLE has {} clauses above the bound of {}",
                self.preferences.len(),
                MAXIMUM_PLANNER_POLICY_CLAUSES
            )));
        }
        if self.preferences.iter().any(|preference| {
            !matches!(
                preference.fact(),
                PlannerFactRef::RealizationCharacteristic(id) if reviewed_style_fact(id)
            )
        }) {
            return invalid("STYLE may reference only the reviewed presentation characteristics");
        }
        Ok(PolicyLayer {
            source: PolicySourceRevision {
                source_id: PolicySourceId::new(self.style_id.as_str()),
                revision: self.revision,
                scope: PolicyScope::NamedStyle,
            },
            hard_predicates: Vec::new(),
            preferences: self
                .preferences
                .iter()
                .cloned()
                .map(RealizationPreference::Fact)
                .collect(),
        })
    }
}

fn selected_advertisement<'a>(
    selection: &ScopedRealizationSelection,
    advertisements: &'a [RealizationAdvertisement],
) -> Option<&'a RealizationAdvertisement> {
    advertisements.iter().find(|item| {
        item.host_id == selection.selection.choice.host_id
            && item.capability_id == selection.selection.choice.capability_id
    })
}

fn preference_outcome(
    advertisement: Option<&RealizationAdvertisement>,
    preference: &PlannerPreference,
) -> StylePreferenceOutcome {
    let PlannerFactRef::RealizationCharacteristic(id) = preference.fact() else {
        return StylePreferenceOutcome::Unavailable;
    };
    let Some(actual) = advertisement
        .into_iter()
        .flat_map(|advertisement| advertisement.characteristics.iter())
        .find(|item| &item.definition.characteristic_id == id)
        .map(|item| fact_value(&item.value))
    else {
        return StylePreferenceOutcome::Unavailable;
    };
    match preference {
        PlannerPreference::PreferEqual { value, .. } => {
            if &actual == value {
                StylePreferenceOutcome::Matched
            } else {
                StylePreferenceOutcome::Unmatched
            }
        }
        PlannerPreference::PreferOrder { values, .. } => {
            if values.first() == Some(&actual) {
                StylePreferenceOutcome::Matched
            } else {
                StylePreferenceOutcome::Unmatched
            }
        }
        PlannerPreference::Minimize { .. } | PlannerPreference::Maximize { .. } => {
            StylePreferenceOutcome::Ranked
        }
    }
}

fn prefer(id: &str, value: PlannerFactValue) -> PlannerPreference {
    PlannerPreference::PreferEqual {
        fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(id)),
        value,
    }
}

pub(crate) fn reviewed_style_fact(id: &CharacteristicId) -> bool {
    matches!(
        id.as_str(),
        PRESENTATION_TEXT_LAYOUT
            | PRESENTATION_DENSITY
            | PRESENTATION_FRAMING
            | PRESENTATION_PALETTE_CLASS
            | PRESENTATION_KEYBOARD_VISIBLE
    )
}

fn category(
    id: &str,
    name: &str,
    help: &str,
    allowed: &[&str],
    value: &str,
) -> RealizationCharacteristic {
    stable_realization_category(
        id,
        name,
        help,
        allowed.iter().map(|item| (*item).into()).collect(),
        false,
        value,
    )
}

fn fact_value(value: &CharacteristicValue) -> PlannerFactValue {
    match value {
        CharacteristicValue::Boolean(value) => PlannerFactValue::Boolean(*value),
        CharacteristicValue::UnsignedQuantity { value, unit } => PlannerFactValue::Quantity {
            value: *value,
            unit: *unit,
        },
        CharacteristicValue::Categorical(value) => PlannerFactValue::Category(value.clone()),
    }
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(detail.into()))
}
