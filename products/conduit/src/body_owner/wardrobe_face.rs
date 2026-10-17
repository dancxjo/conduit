//! An owner-authored Face context for the current wardrobe report. Its content
//! changes do not replace the immutable owner face Plan or fabricate
//! an application Face revision.

use super::Owner;
use conduit_face::{
    Face, FaceInteractionContext, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText, MaskWardrobe, SealedMaskPlotRoute, SelectedMaskPlotRoute,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct RouteDescription {
    route_id: String,
    mask_name: String,
    host_id: String,
}

impl Owner {
    /// The service builds this from its own bounded wardrobe report, so the
    /// spoken Mask reads current owner truth instead of client-authored prose.
    pub(crate) fn wardrobe_reading_face(&self, report: &Value) -> Result<Face, String> {
        let base = self.local_face_snapshot()?;
        if report["schema"] != "conduit.body/owner-mask-wardrobe@1"
            || report["face_id"] != base.identity.as_str()
            || report["face_revision"] != base.revision
            || report["body_id"]
                != serde_json::to_value(&base.basis.body_id).map_err(|error| error.to_string())?
        {
            return Err("owner wardrobe report is not based on the current Face".into());
        }
        let wardrobe: MaskWardrobe = serde_json::from_value(report["wardrobe"].clone())
            .map_err(|error| format!("owner wardrobe state: {error}"))?;
        let routes: Vec<SealedMaskPlotRoute> =
            serde_json::from_value(report["admitted_routes"].clone())
                .map_err(|error| format!("owner wardrobe routes: {error}"))?;
        let descriptions: Vec<RouteDescription> =
            serde_json::from_value(report["route_descriptions"].clone())
                .map_err(|error| format!("owner wardrobe names: {error}"))?;
        let selected: Option<SelectedMaskPlotRoute> =
            serde_json::from_value(report["selected"].clone())
                .map_err(|error| format!("owner wardrobe selection: {error}"))?;
        let owner_plan_id = report["owner_plan_id"]
            .as_str()
            .ok_or("owner wardrobe omitted its face Plan")?;
        let root = "owner/wardrobe";
        let mut subjects = vec![FaceSubject {
            identity: root.into(),
            role: FaceRole::Collection,
            name: "Current Mask wardrobe".into(),
        }];
        let mut relationships = Vec::with_capacity(descriptions.len());
        let mut text = vec![FaceText {
            subject: root.into(),
            text: format!(
                "Wardrobe revision {}. {} worn Masks and {} preferred Masks. {}. {}. {}.",
                wardrobe.revision,
                wardrobe.worn.len(),
                wardrobe.preference.len(),
                if selected.is_some() {
                    "An admitted route is selected"
                } else {
                    "No route is selected"
                },
                if report["show_id"].as_str().is_some() {
                    "Its Show is acknowledged"
                } else {
                    "No current Show is acknowledged"
                },
                match report["reconciliation"]["planning"].as_str() {
                    Some("ReplacementRequired") => "A replacement Plan is required",
                    Some("NotRequired") => "The current Plan remains admitted",
                    _ => "Planning state is unavailable",
                }
            ),
        }];
        if report["fresh_show_required"] == true {
            text.push(FaceText {
                subject: root.into(),
                text: "The selected route needs a fresh acknowledged Show.".into(),
            });
        }
        for (index, description) in descriptions.iter().enumerate() {
            let route = routes
                .iter()
                .find(|route| route.route_id == description.route_id)
                .ok_or("owner wardrobe name has no admitted route")?;
            let identity = format!("owner/wardrobe/route/{index}");
            subjects.push(FaceSubject {
                identity: identity.clone(),
                role: FaceRole::Route,
                name: format!("{} Mask", description.mask_name),
            });
            relationships.push(FaceRelationship {
                source: root.into(),
                target: identity.clone(),
                kind: FaceRelationshipKind::Contains,
            });
            let rank = wardrobe
                .preference
                .iter()
                .position(|plot| plot == &route.mask_plot);
            text.push(FaceText {
                subject: identity,
                text: format!(
                    "{} on Host {}. {}. {}. {}.",
                    if route.currently_available {
                        "Available"
                    } else {
                        "Unavailable"
                    },
                    description.host_id,
                    if wardrobe.worn.contains(&route.mask_plot) {
                        "Worn"
                    } else {
                        "Not worn"
                    },
                    rank.map_or_else(
                        || "Not preferred".into(),
                        |rank| format!("Preference rank {}", rank + 1)
                    ),
                    if selected
                        .as_ref()
                        .is_some_and(|item| item.route_id == route.route_id)
                    {
                        "Selected"
                    } else {
                        "Not selected"
                    }
                ),
            });
        }
        let face = Face::new(
            wardrobe.revision,
            base.basis,
            subjects,
            relationships,
            Vec::new(),
            text,
        )
        .and_then(|face| {
            face.with_interaction_context(FaceInteractionContext {
                identity: format!(
                    "face/context/owner-wardrobe/{}/{}",
                    owner_plan_id, wardrobe.revision
                ),
                basis: Vec::new(),
            })
        })
        .map_err(|error| format!("owner wardrobe Face: {error:?}"))?;
        face.validate()
            .map_err(|error| format!("owner wardrobe Face: {error:?}"))?;
        Ok(face)
    }
}
