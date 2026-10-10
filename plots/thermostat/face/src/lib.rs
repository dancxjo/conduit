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
        basis,
        subjects: vec![
            PresentationSubject {
                identity: "thermostat/device".into(),
                role: PresentationRole::Region,
                name: "Living room".into(),
            },
            PresentationSubject {
                identity: "thermostat/current".into(),
                role: PresentationRole::Status,
                name: "Current temperature".into(),
            },
            PresentationSubject {
                identity: "thermostat/status".into(),
                role: PresentationRole::Status,
                name: "Requested control".into(),
            },
        ],
        relationships: ["thermostat/current", "thermostat/status"]
            .into_iter()
            .map(|target| PresentationRelationship {
                source: "thermostat/device".into(),
                target: target.into(),
                kind: PresentationRelationshipKind::Contains,
            })
            .collect(),
        composition: Vec::new(),
        properties: vec![
            property(
                "thermostat-revision",
                PresentationPropertyValue::Count(state.revision.into()),
            ),
            property(
                "target-decicelsius",
                PresentationPropertyValue::Count(state.target as u64),
            ),
            property(
                "mode",
                PresentationPropertyValue::Text(mode_name(state.mode).into()),
            ),
            property(
                "fan",
                PresentationPropertyValue::Text(
                    match state.fan {
                        Fan::Auto => "auto",
                        Fan::On => "on",
                    }
                    .into(),
                ),
            ),
            property(
                "preset",
                PresentationPropertyValue::Text(
                    match state.preset {
                        Preset::Custom => "custom",
                        Preset::Comfort => "comfort",
                        Preset::Eco => "eco",
                        Preset::Sleep => "sleep",
                    }
                    .into(),
                ),
            ),
            property(
                "sensor-available",
                PresentationPropertyValue::Flag(state.measured.is_some()),
            ),
            property(
                "equipment-confirmed",
                PresentationPropertyValue::Flag(false),
            ),
        ],
        text: vec![
            PresentationText {
                subject: "thermostat/current".into(),
                text: match state.measured {
                    Some(v) => format!("{:.1}°C observed", f64::from(v) / 10.0),
                    None => "Sensor unavailable".into(),
                },
            },
            PresentationText {
                subject: "thermostat/status".into(),
                text: state.demand().into(),
            },
        ],
        actions: Vec::new(),
        disclosures: [
            "thermostat/device",
            "thermostat/current",
            "thermostat/status",
        ]
        .into_iter()
        .map(|subject| PresentationDisclosure {
            subject: subject.into(),
            level: PresentationDisclosureLevel::Primary,
        })
        .collect(),
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
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
            target: "thermostat/device".into(),
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
