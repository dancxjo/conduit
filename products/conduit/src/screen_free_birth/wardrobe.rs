//! Nonvisual commands over the installed owner's one shared Mask wardrobe.
//! Numbered choices are positions in the inspected owner report, resolved to
//! exact route identities before a revision-bound action is submitted.

use conduit_core::{PlanId, PlotIdentity};
use conduit_presentation::MaskWardrobeAction;
use serde_json::Value;
use std::{io::Write, path::Path};

use super::{command_input::CommandInput, wardrobe_speech::Announcement};

enum Command<'a> {
    Inspect,
    Wear(&'a str),
    Doff(&'a str),
    Prefer(Vec<&'a str>),
}

fn parse(line: &str) -> Option<Result<Command<'_>, &'static str>> {
    if line == "wardrobe" {
        return Some(Ok(Command::Inspect));
    }
    let rest = line.strip_prefix("wardrobe ")?;
    let mut words = rest.split_ascii_whitespace();
    let action = words.next()?;
    let routes = words.collect::<Vec<_>>();
    Some(match action {
        "wear" if routes.len() == 1 => Ok(Command::Wear(routes[0])),
        "doff" if routes.len() == 1 => Ok(Command::Doff(routes[0])),
        "prefer" if !routes.is_empty() && routes.len() <= 16 => Ok(Command::Prefer(routes)),
        "wear" | "doff" | "prefer" => Err(
            "give numbered choices or exact route IDs from wardrobe; prefer accepts one to sixteen",
        ),
        _ => Err("use wardrobe, wardrobe wear/doff CHOICE, or wardrobe prefer CHOICE ..."),
    })
}

pub(super) fn handle<I: CommandInput>(
    state_dir: &Path,
    line: &str,
    last_report: &mut Option<Value>,
    mut speaker: Option<&mut Announcement<'_, I>>,
    output: &mut impl Write,
) -> Result<bool, String> {
    let Some(command) = parse(line) else {
        return Ok(false);
    };
    let command = match command {
        Ok(command) => command,
        Err(reason) => {
            writeln!(output, "Wardrobe refused: {reason}").map_err(|error| error.to_string())?;
            speak_refusal(speaker.as_deref_mut(), reason, output)?;
            return Ok(true);
        }
    };
    if matches!(command, Command::Inspect) {
        match crate::durable_host_control::owner_wardrobe::report(state_dir, None, 0, None) {
            Ok(report) => {
                write_report(output, &report)?;
                *last_report = if speak_report(speaker.as_deref_mut(), &report, output)? {
                    Some(report)
                } else {
                    None
                };
            }
            Err(reason) => {
                *last_report = None;
                writeln!(output, "Wardrobe refused: {reason}")
                    .map_err(|error| error.to_string())?;
                speak_refusal(speaker.as_deref_mut(), &reason, output)?;
            }
        }
        return Ok(true);
    }
    let Some(before) = last_report.as_ref() else {
        writeln!(
            output,
            "Wardrobe refused: inspect wardrobe before choosing a route."
        )
        .map_err(|error| error.to_string())?;
        speak_refusal(
            speaker.as_deref_mut(),
            "inspect wardrobe before choosing a route",
            output,
        )?;
        return Ok(true);
    };
    let action = match command {
        Command::Inspect => unreachable!("inspection returned above"),
        Command::Wear(route) => {
            plot_for_route(&before, route).map(|plot| Some(MaskWardrobeAction::Wear(plot)))
        }
        Command::Doff(route) => {
            plot_for_route(&before, route).map(|plot| Some(MaskWardrobeAction::Doff(plot)))
        }
        Command::Prefer(routes) => routes
            .into_iter()
            .map(|route| plot_for_route(&before, route))
            .collect::<Result<Vec<_>, _>>()
            .map(|plots| Some(MaskWardrobeAction::Prefer(plots))),
    };
    let action = match action {
        Ok(action) => action,
        Err(reason) => {
            writeln!(output, "Wardrobe refused: {reason}").map_err(|error| error.to_string())?;
            speak_refusal(speaker.as_deref_mut(), &reason, output)?;
            return Ok(true);
        }
    };
    let report = if let Some(action) = action {
        let plan: PlanId = serde_json::from_value(before["owner_plan_id"].clone())
            .map_err(|_| "owner wardrobe omitted its exact Plan".to_string())?;
        let revision = before["wardrobe"]["revision"]
            .as_u64()
            .ok_or("owner wardrobe omitted its revision")?;
        match crate::durable_host_control::owner_wardrobe::report(
            state_dir,
            Some(plan),
            revision,
            Some(action),
        ) {
            Ok(report) => report,
            Err(reason) => {
                *last_report = None;
                writeln!(
                    output,
                    "Wardrobe refused: {reason}; inspect wardrobe again before choosing."
                )
                .map_err(|error| error.to_string())?;
                speak_refusal(speaker.as_deref_mut(), &reason, output)?;
                return Ok(true);
            }
        }
    } else {
        unreachable!("all non-inspection commands are actions")
    };
    write_report(output, &report)?;
    *last_report = if speak_report(speaker.as_deref_mut(), &report, output)? {
        Some(report)
    } else {
        None
    };
    Ok(true)
}

fn speak_refusal<I: CommandInput>(
    speaker: Option<&mut Announcement<'_, I>>,
    reason: &str,
    output: &mut impl Write,
) -> Result<(), String> {
    if let Some(speaker) = speaker {
        speaker.speak(
            None,
            &[format!(
                "Wardrobe refused. {reason}. Inspect wardrobe again if you need current choices."
            )],
            output,
        )?;
    }
    Ok(())
}

fn speak_report<I: CommandInput>(
    speaker: Option<&mut Announcement<'_, I>>,
    report: &Value,
    output: &mut impl Write,
) -> Result<bool, String> {
    match speaker {
        Some(speaker) => speaker.speak(Some(report), &spoken_report_lines(report)?, output),
        None => Ok(true),
    }
}

fn spoken_report_lines(report: &Value) -> Result<Vec<String>, String> {
    let routes = report["admitted_routes"]
        .as_array()
        .ok_or("owner wardrobe omitted admitted routes")?;
    let mut lines = vec![format!(
        "Owner wardrobe revision {}. Worn Masks: {}. Preference, first preferred first: {}.",
        report["wardrobe"]["revision"],
        plot_labels(report, &report["wardrobe"]["worn"]),
        plot_labels(report, &report["wardrobe"]["preference"]),
    )];
    for (index, route) in routes.iter().enumerate() {
        let name = route_name(report, route);
        let available = if route["currently_available"] == true {
            "available"
        } else {
            "unavailable"
        };
        let selected = if report["selected"]["route_id"] == route["route_id"] {
            "selected"
        } else {
            "not selected"
        };
        lines.push(format!(
            "Choice {}: {name}, {available}, {selected}.",
            index + 1
        ));
    }
    let show = if report["show_id"].is_null() {
        "No current acknowledged Show"
    } else {
        "A current Show is acknowledged"
    };
    let planning = if report["reconciliation"]["planning"] == "ReplacementRequired" {
        "Replacement planning is required"
    } else {
        "The current owner Plan needs no replacement"
    };
    lines.push(format!("{show}. {planning}. Use wardrobe wear, doff, or prefer followed by a choice number. Preference alone does not displace a current available Show."));
    Ok(lines)
}

fn route_name<'a>(report: &'a Value, route: &Value) -> &'a str {
    let route_id = route["route_id"].as_str().unwrap_or("unidentified");
    report["route_descriptions"]
        .as_array()
        .and_then(|descriptions| {
            descriptions
                .iter()
                .find(|item| item["route_id"] == route_id)
        })
        .and_then(|description| description["mask_name"].as_str())
        .unwrap_or("unnamed Mask")
}

fn plot_for_route(report: &Value, choice: &str) -> Result<PlotIdentity, String> {
    let routes = report["admitted_routes"]
        .as_array()
        .ok_or("owner wardrobe omitted admitted routes")?;
    let route = if let Ok(number) = choice.parse::<usize>() {
        number.checked_sub(1).and_then(|index| routes.get(index))
    } else {
        routes.iter().find(|route| route["route_id"] == choice)
    }
    .ok_or_else(|| format!("choice {choice} is not admitted by this owner Plan"))?;
    serde_json::from_value(route["mask_plot"].clone())
        .map_err(|_| "owner wardrobe route omitted its Mask Plot identity".into())
}

fn write_report(output: &mut impl Write, report: &Value) -> Result<(), String> {
    let routes = report["admitted_routes"]
        .as_array()
        .ok_or("owner wardrobe omitted admitted routes")?;
    writeln!(
        output,
        "Owner Body {}. Face {} revision {}. Owner Plan {}. Wardrobe revision {}.",
        report["body_id"].as_str().unwrap_or("unidentified"),
        report["face_id"].as_str().unwrap_or("unidentified"),
        report["face_revision"],
        report["owner_plan_id"].as_str().unwrap_or("unidentified"),
        report["wardrobe"]["revision"]
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "Worn Masks: {}. Preference, first preferred first: {}.",
        plot_labels(report, &report["wardrobe"]["worn"]),
        plot_labels(report, &report["wardrobe"]["preference"])
    )
    .map_err(|error| error.to_string())?;
    if routes.is_empty() {
        writeln!(output, "No route is admitted by the current owner Plan.")
            .map_err(|error| error.to_string())?;
    }
    for (index, route) in routes.iter().enumerate() {
        let availability = if route["currently_available"] == true {
            "available"
        } else {
            "unavailable"
        };
        let route_id = route["route_id"].as_str().unwrap_or("unidentified");
        let name = report["route_descriptions"]
            .as_array()
            .and_then(|descriptions| {
                descriptions
                    .iter()
                    .find(|item| item["route_id"] == route_id)
            })
            .and_then(|description| description["mask_name"].as_str())
            .unwrap_or("unnamed Mask");
        writeln!(
            output,
            "Choice {}: {name}, {availability}. Exact route {route_id}, sealed by Plan {}. Checked Plot {}.",
            index + 1,
            route["plan_id"].as_str().unwrap_or("unidentified"),
            route["mask_plot"]["checked_plot_id"]
                .as_str()
                .unwrap_or("unidentified"),
        )
        .map_err(|error| error.to_string())?;
    }
    writeln!(
        output,
        "Selected route: {}. Current Show: {}. Replacement planning: {}. Fresh Show needed: {}.",
        report["selected"]["route_id"].as_str().unwrap_or("none"),
        report["show_id"].as_str().unwrap_or("none"),
        report["reconciliation"]["planning"],
        report["fresh_show_required"]
    )
    .map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())
}

fn plot_labels(report: &Value, plots: &Value) -> String {
    let Some(plots) = plots.as_array() else {
        return "unknown".into();
    };
    if plots.is_empty() {
        return "none".into();
    }
    plots
        .iter()
        .map(|plot| {
            report["admitted_routes"]
                .as_array()
                .and_then(|routes| routes.iter().find(|route| route["mask_plot"] == *plot))
                .and_then(|route| route["route_id"].as_str())
                .and_then(|route_id| {
                    report["route_descriptions"]
                        .as_array()
                        .and_then(|descriptions| {
                            descriptions
                                .iter()
                                .find(|item| item["route_id"] == route_id)
                        })
                        .and_then(|item| item["mask_name"].as_str())
                })
                .or_else(|| plot["checked_plot_id"].as_str())
                .unwrap_or("unidentified Mask")
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoInput;
    impl CommandInput for NoInput {
        fn next(&mut self) -> super::super::command_input::InputEvent {
            super::super::command_input::InputEvent::Eof
        }
    }

    #[test]
    fn wardrobe_input_needs_exact_route_and_never_accepts_implicit_preference() {
        assert!(matches!(parse("wardrobe"), Some(Ok(Command::Inspect))));
        assert!(matches!(
            parse("wardrobe wear route/one"),
            Some(Ok(Command::Wear("route/one")))
        ));
        assert!(matches!(
            parse("wardrobe prefer route/one route/two"),
            Some(Ok(Command::Prefer(_)))
        ));
        assert!(matches!(parse("wardrobe prefer"), Some(Err(_))));
        assert!(matches!(parse("wardrobe doff"), Some(Err(_))));
        assert!(parse("activate").is_none());
    }

    #[test]
    fn unknown_route_cannot_become_a_plot_action() {
        let report = serde_json::json!({"admitted_routes": []});
        assert!(plot_for_route(&report, "route/absent").is_err());
    }

    #[test]
    fn exact_admitted_route_yields_the_owners_plot_identity() {
        let plot = PlotIdentity {
            source_document_id: "source/selected".into(),
            checked_plot_id: "checked/selected".into(),
            expanded_plot_id: "expanded/selected".into(),
        };
        let report = serde_json::json!({
            "admitted_routes": [{"route_id": "route/selected", "mask_plot": plot}]
        });
        assert_eq!(plot_for_route(&report, "route/selected").unwrap(), plot);
        assert_eq!(plot_for_route(&report, "1").unwrap(), plot);
        assert!(plot_for_route(&report, "2").is_err());
        assert!(plot_for_route(&report, "route/stale").is_err());
    }

    #[test]
    fn change_requires_a_report_the_person_inspected() {
        let mut report = None;
        let mut output = Vec::new();
        assert!(handle::<NoInput>(
            Path::new("/no-live-owner-needed"),
            "wardrobe prefer route/old",
            &mut report,
            None,
            &mut output,
        )
        .unwrap());
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("inspect wardrobe before choosing"));
    }

    #[test]
    fn spoken_choices_derive_from_owner_report_and_preserve_order() {
        let first = serde_json::json!({"source_document_id":"source/one","checked_plot_id":"checked/one","expanded_plot_id":"expanded/one"});
        let second = serde_json::json!({"source_document_id":"source/two","checked_plot_id":"checked/two","expanded_plot_id":"expanded/two"});
        let report = serde_json::json!({
            "wardrobe":{"revision":4,"worn":[first.clone(),second.clone()],"preference":[second.clone(),first.clone()]},
            "admitted_routes":[
                {"route_id":"route/long-one","mask_plot":first,"currently_available":false},
                {"route_id":"route/long-two","mask_plot":second,"currently_available":true}
            ],
            "route_descriptions":[
                {"route_id":"route/long-one","mask_name":"Terminal"},
                {"route_id":"route/long-two","mask_name":"Direct speech"}
            ],
            "selected":{"route_id":"route/long-two"},
            "show_id":"show/current",
            "reconciliation":{"planning":"NotRequired"}
        });
        let lines = spoken_report_lines(&report).unwrap();
        assert!(lines[0].contains("Direct speech, Terminal"));
        assert_eq!(lines[1], "Choice 1: Terminal, unavailable, not selected.");
        assert_eq!(lines[2], "Choice 2: Direct speech, available, selected.");
        assert!(lines[3].contains("current Show is acknowledged"));
        assert!(lines[3].contains("needs no replacement"));
    }
}
