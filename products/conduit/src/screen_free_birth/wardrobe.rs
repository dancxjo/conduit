//! Nonvisual controls over the installed owner's exact admitted Mask wardrobe.
//! The client names a human-readable offered Mask; the owner alone applies it
//! against the Plan and revision last reported to this session.

use std::{io::Write, path::Path};

use conduit_core::{HostAdvertisement, PlanId, PlotIdentity};
use conduit_presentation::{
    MaskWardrobe, MaskWardrobeAction, Presentation, SealedMaskPlotRoute, SelectedMaskPlotRoute,
};
use serde::Deserialize;
use serde_json::Value;

use crate::durable_host_control;

pub(super) fn report_refusal(refusal: &str, output: &mut impl Write) -> Result<(), String> {
    if refusal == "owner wardrobe routes: StaleBodyOrFace" {
        writeln!(output, "The Body or Face changed. Its previous Mask Plan is stale, so no route from that Plan can be used now. Read the current Face; inspect the wardrobe after a Host offers a fresh Mask.")
            .map_err(|error| error.to_string())?;
        writeln!(
            output,
            "{}",
            serde_json::json!({
                "schema": "conduit.body/screen-free-wardrobe-refusal@1",
                "code": "stale-body-or-face",
            })
        )
        .map_err(|error| error.to_string())
    } else {
        writeln!(
            output,
            "Wardrobe refused: {refusal}. Enter wardrobe to inspect current owner state."
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone)]
pub(super) struct WardrobeReadout {
    raw: Value,
    face: Result<Presentation, String>,
    plan_id: PlanId,
    wardrobe: MaskWardrobe,
    routes: Vec<SealedMaskPlotRoute>,
    descriptions: Vec<RouteDescription>,
    selected: Option<SelectedMaskPlotRoute>,
}

#[derive(Clone, Deserialize)]
struct RouteDescription {
    route_id: String,
    mask_name: String,
    host_id: String,
}

impl WardrobeReadout {
    fn decode(raw: Value, face: Result<Presentation, String>) -> Result<Self, String> {
        if raw["schema"] != "conduit.body/owner-mask-wardrobe@1" {
            return Err("owner returned another wardrobe schema".into());
        }
        let field = |name| {
            raw.get(name)
                .cloned()
                .ok_or_else(|| format!("owner wardrobe omitted {name}"))
        };
        let plan_id = serde_json::from_value(field("owner_plan_id")?)
            .map_err(|error| format!("owner wardrobe Plan: {error}"))?;
        let wardrobe: MaskWardrobe = serde_json::from_value(field("wardrobe")?)
            .map_err(|error| format!("owner wardrobe state: {error}"))?;
        let routes = serde_json::from_value(field("admitted_routes")?)
            .map_err(|error| format!("owner wardrobe routes: {error}"))?;
        let descriptions = serde_json::from_value(field("route_descriptions")?)
            .map_err(|error| format!("owner wardrobe names: {error}"))?;
        let selected = serde_json::from_value(field("selected")?)
            .map_err(|error| format!("owner wardrobe selection: {error}"))?;
        if face.as_ref().is_ok_and(|reading| {
            reading.revision != wardrobe.revision || reading.basis.body_id.is_none()
        }) {
            return Err("owner wardrobe Face differs from its current report".into());
        }
        Ok(Self {
            raw,
            face,
            plan_id,
            wardrobe,
            routes,
            descriptions,
            selected,
        })
    }

    fn offered_mask(&self, name: &str) -> Result<PlotIdentity, String> {
        let mut found = None;
        for description in self
            .descriptions
            .iter()
            .filter(|description| description.mask_name == name)
        {
            let route = self
                .routes
                .iter()
                .find(|route| route.route_id == description.route_id)
                .ok_or("owner wardrobe name has no admitted route")?;
            match &found {
                None => found = Some(route.mask_plot.clone()),
                Some(plot) if plot == &route.mask_plot => {}
                Some(_) => {
                    return Err(format!(
                        "Mask name {name} is ambiguous; inspect exact routes"
                    ))
                }
            }
        }
        found.ok_or_else(|| format!("Mask {name} is not in the current owner Plan"))
    }

    fn action(&self, line: &str) -> Result<MaskWardrobeAction, String> {
        let (verb, names) = line
            .strip_prefix("wardrobe ")
            .and_then(|rest| rest.split_once(' '))
            .ok_or("enter wardrobe wear/doff/prefer followed by an offered Mask name")?;
        if names.is_empty() {
            return Err("name an offered Mask from wardrobe inspection".into());
        }
        match verb {
            "wear" => Ok(MaskWardrobeAction::Wear(self.offered_mask(names)?)),
            "doff" => Ok(MaskWardrobeAction::Doff(self.offered_mask(names)?)),
            "prefer" => {
                let names: Vec<_> = names.split(',').map(str::trim).collect();
                if names.len() > 16 || names.iter().any(|name| name.is_empty()) {
                    return Err("prefer needs one to sixteen offered Mask names".into());
                }
                let plots = names
                    .into_iter()
                    .map(|name| self.offered_mask(name))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(MaskWardrobeAction::Prefer(plots))
            }
            _ => Err("enter wardrobe wear, doff, or prefer".into()),
        }
    }

    fn mask_name(&self, plot: &PlotIdentity) -> String {
        self.routes
            .iter()
            .find(|route| &route.mask_plot == plot)
            .and_then(|route| {
                self.descriptions
                    .iter()
                    .find(|description| description.route_id == route.route_id)
            })
            .map_or_else(
                || "unlisted Mask".into(),
                |description| description.mask_name.clone(),
            )
    }

    fn write(&self, output: &mut impl Write) -> Result<(), String> {
        let names = |plots: &[PlotIdentity]| {
            plots
                .iter()
                .map(|plot| self.mask_name(plot))
                .collect::<Vec<_>>()
                .join(", ")
        };
        writeln!(
            output,
            "Owner wardrobe revision {}. Worn: {}. Preference: {}. Selected: {}. Current acknowledged Show: {}. Planning: {}.",
            self.wardrobe.revision,
            if self.wardrobe.worn.is_empty() { "none".into() } else { names(&self.wardrobe.worn) },
            if self.wardrobe.preference.is_empty() { "none".into() } else { names(&self.wardrobe.preference) },
            self.selected.as_ref().map_or("none", |selected| selected.route_id.as_str()),
            self.raw["show_id"].as_str().unwrap_or("none"),
            self.raw["reconciliation"]["planning"].as_str().unwrap_or("unknown"),
        )
        .map_err(|error| error.to_string())?;
        for description in &self.descriptions {
            let route = self
                .routes
                .iter()
                .find(|route| route.route_id == description.route_id)
                .ok_or("owner wardrobe route description differs from its Plan")?;
            writeln!(
                output,
                "  {} on Host {}: {}. Route {}.",
                description.mask_name,
                description.host_id,
                if route.currently_available {
                    "available"
                } else {
                    "unavailable"
                },
                route.route_id,
            )
            .map_err(|error| error.to_string())?;
        }
        if self.raw["fresh_show_required"] == true {
            writeln!(
                output,
                "The selected route needs a fresh acknowledged Show."
            )
            .map_err(|error| error.to_string())?;
        }
        writeln!(
            output,
            "Owner wardrobe report is terminal text. Any selected-speaker Play has a separate completion receipt."
        )
        .map_err(|error| error.to_string())
    }
}

fn current_report(
    state_dir: &Path,
    face: &Presentation,
    host: &HostAdvertisement,
    basis: Option<&WardrobeReadout>,
    action: Option<MaskWardrobeAction>,
) -> Result<WardrobeReadout, String> {
    let body_id = face
        .basis
        .body_id
        .clone()
        .ok_or("owner wardrobe needs a current Body")?;
    let (report, reading) = durable_host_control::local_wardrobe(
        state_dir,
        body_id,
        face.identity.as_str().into(),
        face.revision,
        host.clone(),
        basis.map(|readout| readout.plan_id.clone()),
        basis.map_or(0, |readout| readout.wardrobe.revision),
        action,
    )?;
    WardrobeReadout::decode(report, reading)
}

pub(super) fn command(
    line: &str,
    state_dir: &Path,
    face: &Presentation,
    host: &HostAdvertisement,
    prior: &mut Option<WardrobeReadout>,
    output: &mut impl Write,
) -> Result<Option<Presentation>, String> {
    let report = if line == "wardrobe" {
        current_report(state_dir, face, host, None, None)?
    } else {
        let basis = prior
            .as_ref()
            .ok_or("inspect wardrobe before changing it")?;
        let action = basis.action(line)?;
        current_report(state_dir, face, host, Some(basis), Some(action))?
    };
    report.write(output)?;
    let face = match &report.face {
        Ok(face) => Some(face.clone()),
        Err(reason) => {
            writeln!(output, "Owner wardrobe changed or was inspected, but its spoken Face is unavailable: {reason}. No speaker Play was made for this report.")
                .map_err(|error| error.to_string())?;
            None
        }
    };
    *prior = Some(report);
    Ok(face)
}
