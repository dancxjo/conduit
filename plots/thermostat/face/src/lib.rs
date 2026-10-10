#![no_std]
//! Thermostat-owned semantic contribution. Masks own appearance, never settings.
extern crate alloc;
use alloc::{format, vec, vec::Vec};
use conduit_presentation::*;
use conduit_thermostat_plot::*;

pub fn fragment(
    state: &ThermostatState,
    basis: PresentationContributionBasis,
    actions_admitted: bool,
) -> Result<PresentationFragment, &'static str> {
    state.validate().map_err(|_| "invalid thermostat state")?;
    let mut fragment = PresentationFragment {
        basis: basis.clone(),
        subjects: vec![
            subject("thermostat/device", PresentationRole::Region, "Living room"),
            subject(
                "thermostat/target",
                PresentationRole::Info,
                "Target temperature",
            ),
            subject(
                "thermostat/current",
                PresentationRole::Status,
                "Current temperature",
            ),
            subject(
                "thermostat/status",
                PresentationRole::Status,
                "Requested control",
            ),
        ],
        relationships: [
            "thermostat/target",
            "thermostat/current",
            "thermostat/status",
        ]
        .into_iter()
        .map(|target| contains("thermostat/device", target))
        .collect(),
        composition: Vec::new(),
        properties: vec![property(
            "thermostat-revision",
            PresentationPropertyValue::Count(state.revision.into()),
        )],
        text: vec![
            PresentationText {
                subject: "thermostat/current".into(),
                text: match state.measured {
                    Some(_) => "Observed temperature".into(),
                    None => "Sensor unavailable".into(),
                },
            },
            PresentationText {
                subject: "thermostat/status".into(),
                text: state.demand().into(),
            },
            PresentationText {
                subject: "thermostat/device".into(),
                text: "Requested settings; equipment operation is unconfirmed.".into(),
            },
        ],
        actions: Vec::new(),
        disclosures: Vec::new(),
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
    for (subject, value) in [
        ("thermostat/target", Some(state.target)),
        ("thermostat/current", state.measured),
    ] {
        let Some(value) = value else {
            continue;
        };
        let quantity = conduit_core::Quantity::from_decimal(
            i128::from(value),
            -1,
            conduit_core::Unit::Celsius,
        )
        .map_err(|_| "invalid exact Celsius quantity")?;
        let contract = conduit_core::CheckedValueContract::new(
            conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID),
            conduit_core::QUANTITY_ENCODED_LEN as u32,
            Vec::new(),
        )
        .map_err(|_| "invalid temperature value contract")?;
        fragment.properties.push(PresentationProperty {
            subject: subject.into(),
            name: "value".into(),
            value: PresentationPropertyValue::TypedValue {
                contract,
                bytes: quantity.encode().to_vec(),
            },
        });
    }
    for (group, name, options, selected) in [
        (
            "thermostat/mode",
            "Mode",
            &ACTIONS[2..6],
            mode_name(state.mode),
        ),
        (
            "thermostat/fan",
            "Fan",
            &ACTIONS[6..8],
            match state.fan {
                Fan::Auto => "auto",
                Fan::On => "on",
            },
        ),
        (
            "thermostat/preset",
            "Preset",
            &ACTIONS[8..11],
            match state.preset {
                Preset::Custom => "custom",
                Preset::Comfort => "comfort",
                Preset::Eco => "eco",
                Preset::Sleep => "sleep",
            },
        ),
    ] {
        fragment.subjects.push(subject(
            group,
            PresentationRole::Semantic(conduit_core::kind_id(CHOICE_GROUP_ROLE)),
            name,
        ));
        fragment
            .relationships
            .push(contains("thermostat/device", group));
        fragment.properties.push(PresentationProperty {
            subject: group.into(),
            name: "choice-multiplicity".into(),
            value: PresentationPropertyValue::Text("exclusive".into()),
        });
        fragment.text.push(PresentationText {
            subject: group.into(),
            text: format!("{} selected", selected),
        });
        for (id, label) in options {
            fragment
                .subjects
                .push(subject(id, PresentationRole::Action, label));
            fragment.relationships.push(contains(group, id));
            fragment.properties.push(PresentationProperty {
                subject: (*id).into(),
                name: "selected".into(),
                value: PresentationPropertyValue::Flag(id.rsplit('.').next() == Some(selected)),
            });
        }
    }
    fragment.disclosures = fragment
        .subjects
        .iter()
        .map(|subject| PresentationDisclosure {
            subject: subject.identity.clone(),
            level: PresentationDisclosureLevel::Primary,
        })
        .collect();
    for (id, label) in ACTIONS {
        let command = command_for_action(state, id)?;
        let availability = if !actions_admitted {
            unavailable(
                "thermostat-return-route-unavailable",
                "No admitted action return route.",
            )
        } else {
            match state.apply(command) {
                Ok(_) => PresentationActionAvailability::Available,
                Err(Refusal::RevisionExhausted) => unavailable(
                    "thermostat-revision-exhausted",
                    "The settings revision has reached its admitted capacity.",
                ),
                Err(Refusal::TargetRange) => unavailable(
                    "thermostat-target-limit",
                    "The target has reached its allowed temperature limit.",
                ),
                Err(_) => unavailable(
                    "thermostat-command-refused",
                    "This command is refused by the thermostat state contract.",
                ),
            }
        };
        fragment.actions.push(PresentationAction {
            identity: id.into(),
            intent: format!("{id}@1"),
            target: if id == "thermostat.lower" || id == "thermostat.raise" {
                "thermostat/target".into()
            } else {
                id.into()
            },
            name: label.into(),
            arguments: Vec::new(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability,
        });
    }
    fragment
        .validate_bounds()
        .map_err(|_| "thermostat Face exceeds bounds")?;
    Ok(fragment)
}
/// Renderer-neutral choice grouping; multiplicity and selection remain Face facts.
pub const CHOICE_GROUP_ROLE: &str = "conduit.presentation/choice-group@1";
fn subject(identity: &str, role: PresentationRole, name: &str) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role,
        name: name.into(),
    }
}
fn contains(source: &str, target: &str) -> PresentationRelationship {
    PresentationRelationship {
        source: source.into(),
        target: target.into(),
        kind: PresentationRelationshipKind::Contains,
    }
}
mod interaction;
pub use interaction::{thermostat_command_from_contributed_interaction, ThermostatFaceError};
#[cfg(test)]
mod tests;

fn property(name: &str, value: PresentationPropertyValue) -> PresentationProperty {
    PresentationProperty {
        subject: "thermostat/device".into(),
        name: name.into(),
        value,
    }
}
fn unavailable(code: &str, message: &str) -> PresentationActionAvailability {
    PresentationActionAvailability::Unavailable {
        reason_code: code.into(),
        explanation: message.into(),
    }
}
fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "off",
        Mode::Heat => "heat",
        Mode::Cool => "cool",
        Mode::Auto => "auto",
    }
}
const ACTIONS: [(&str, &str); 11] = [
    ("thermostat.lower", "Lower target by 0.5°C"),
    ("thermostat.raise", "Raise target by 0.5°C"),
    ("thermostat.mode.off", "Off"),
    ("thermostat.mode.heat", "Heat"),
    ("thermostat.mode.cool", "Cool"),
    ("thermostat.mode.auto", "Auto"),
    ("thermostat.fan.auto", "Auto"),
    ("thermostat.fan.on", "On"),
    ("thermostat.preset.comfort", "Comfort"),
    ("thermostat.preset.eco", "Eco"),
    ("thermostat.preset.sleep", "Sleep"),
];
/// Resolve only an offered thermostat action. The caller validates the exact
/// current revision and delivered action contract before admitting these bytes.
pub fn command_for_action(state: &ThermostatState, action: &str) -> Result<Command, &'static str> {
    Ok(match action {
        "thermostat.lower" => Command::SetTarget(state.target - TARGET_STEP),
        "thermostat.raise" => Command::SetTarget(state.target + TARGET_STEP),
        "thermostat.mode.off" => Command::SetMode(Mode::Off),
        "thermostat.mode.heat" => Command::SetMode(Mode::Heat),
        "thermostat.mode.cool" => Command::SetMode(Mode::Cool),
        "thermostat.mode.auto" => Command::SetMode(Mode::Auto),
        "thermostat.fan.auto" => Command::SetFan(Fan::Auto),
        "thermostat.fan.on" => Command::SetFan(Fan::On),
        "thermostat.preset.comfort" => Command::SetPreset(Preset::Comfort),
        "thermostat.preset.eco" => Command::SetPreset(Preset::Eco),
        "thermostat.preset.sleep" => Command::SetPreset(Preset::Sleep),
        _ => return Err("unknown thermostat action"),
    })
}
