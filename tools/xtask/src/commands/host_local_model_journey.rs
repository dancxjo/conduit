use std::{collections::BTreeMap, path::Path, process::Command};

use conduit_body::{BodyLifecycleEvent, WakeLifecycleEvent};

use super::host_local_model::PreparedOrifinaJourney;
use crate::commands::body_journey_track::{self, TrackIdentities, TrackSource};

#[path = "host_local_model_documentary.rs"]
mod documentary;

pub(super) fn write(
    journey: &PreparedOrifinaJourney,
    receipt: &conduit_std_host::local_model_proof::LocalModelLiveProofReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
    let evidence = &journey.receipt.biography;
    let body_sign = |select: fn(&BodyLifecycleEvent) -> bool| {
        evidence
            .body
            .events
            .iter()
            .find(|event| select(event))
            .map(|event| event.sign_id().as_str().to_owned())
            .ok_or("Orifina biography lacks a required Body sign")
    };
    let born = body_sign(|event| matches!(event, BodyLifecycleEvent::Born { .. }))?;
    let workload = body_sign(|event| matches!(event, BodyLifecycleEvent::FormAdmitted { .. }))?;
    let lulled = body_sign(|event| matches!(event, BodyLifecycleEvent::LullRetained { .. }))?;
    let fulfilled = body_sign(|event| matches!(event, BodyLifecycleEvent::Fulfilled { .. }))?;
    let wake = evidence
        .wakes
        .first()
        .and_then(|wake| wake.sign_ids.first())
        .ok_or("Orifina biography lacks its Wake sign")?
        .as_str()
        .to_owned();
    let failed = evidence
        .wakes
        .iter()
        .find(|wake| {
            wake.events
                .iter()
                .any(|event| matches!(event, WakeLifecycleEvent::Failed { .. }))
        })
        .and_then(|wake| wake.sign_ids.last())
        .ok_or("Orifina biography lacks its fault sign")?
        .as_str()
        .to_owned();
    let repaired = evidence
        .wakes
        .last()
        .and_then(|wake| wake.sign_ids.last())
        .ok_or("Orifina biography lacks its repair sign")?
        .as_str()
        .to_owned();
    let request = journey
        .requests
        .iter()
        .find(|request| {
            let basis = &request.semantic_data.presentation.basis;
            basis.source_document_id.is_some()
                && basis.checked_form_id.is_some()
                && basis.expanded_form_id.is_some()
                && basis.plan_id.is_some()
        })
        .ok_or("Orifina live proof lacks a planned Face for its spoken Mask")?;
    let manifestation = receipt
        .presenter_requests
        .iter()
        .find(|candidate| candidate.request_identity == request.request_identity)
        .ok_or("Orifina live proof lacks its planned generative Mask Show")?;
    let presentation = request.semantic_data.presentation.clone();
    let retained = manifestation.manifestation.clone();
    let initial = conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
        "spoken-initial",
        "orifina-spoken-initial",
        presentation.clone(),
        retained.clone(),
    )?;
    let alternate = conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
        "spoken-generative",
        "orifina-spoken-alternate",
        presentation.clone(),
        retained.clone(),
    )?;
    let replacement_alternate =
        conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
            "spoken-generative",
            "orifina-spoken-replanned",
            presentation.clone(),
            retained.clone(),
        )?;
    let restored = conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
        "spoken-initial",
        "orifina-spoken-restored",
        presentation.clone(),
        retained,
    )?;
    let initial_routes = conduit_std_host::spoken_mask_journey::admit_spoken_mask_routes(
        &[initial.clone(), alternate.clone()],
        "orifina-spoken-initial-plan",
    )?;
    let replacement_routes =
        conduit_std_host::spoken_mask_journey::admit_replacement_spoken_mask_routes(
            &[replacement_alternate.clone(), restored.clone()],
            "orifina-spoken-replacement-plan",
            &[true, true],
            &initial_routes.wake,
        )?;
    use conduit_presentation::MaskJourneyAction as A;
    struct HostedSpokenJourney {
        presentation_id: String,
        initial: Vec<conduit_std_host::spoken_mask_journey::SpokenMaskExecution>,
        replacement: Vec<conduit_std_host::spoken_mask_journey::SpokenMaskExecution>,
        initial_routes: conduit_std_host::spoken_mask_journey::SpokenMaskRouteSet,
        replacement_routes: conduit_std_host::spoken_mask_journey::SpokenMaskRouteSet,
        control: conduit_presentation::MaskWardrobeControl,
        current: Option<(bool, usize)>,
    }

    impl HostedSpokenJourney {
        fn observation(
            &self,
            action: A,
            event: &str,
            receipts: Vec<String>,
        ) -> conduit_std_host::spoken_mask_journey::SpokenMaskJourneyObservation {
            let retains_fresh_show = matches!(
                action,
                A::InspectInitialShow
                    | A::PreferAlternateMask
                    | A::InspectReplannedShow
                    | A::InspectRestoredShow
            );
            let execution = retains_fresh_show
                .then(|| {
                    self.current.map(|(replacement, index)| {
                        if replacement {
                            &self.replacement[index]
                        } else {
                            &self.initial[index]
                        }
                    })
                })
                .flatten();
            conduit_std_host::spoken_mask_journey::SpokenMaskJourneyObservation {
                action_id: action.id().into(),
                concrete_event: event.into(),
                presentation_id: self.presentation_id.clone(),
                selected_mask_form_id: self
                    .control
                    .selected
                    .as_ref()
                    .map(|selected| selected.mask_form.checked_form_id.as_str().into()),
                plan_id: self.control.active_plan_id.as_str().into(),
                selected_route_id: self
                    .control
                    .selected
                    .as_ref()
                    .map(|selected| selected.route_id.clone()),
                show_id: execution.map(|item| item.shown.show.show_id.as_str().into()),
                receipt_ids: receipts,
            }
        }

        fn artifact_receipts(&self) -> Result<Vec<String>, String> {
            let execution = self
                .current
                .map(|(replacement, index)| {
                    if replacement {
                        &self.replacement[index]
                    } else {
                        &self.initial[index]
                    }
                })
                .ok_or("hosted spoken journey has no current Show")?;
            Ok(vec![
                execution.shown.artifact.artifact_identity.clone(),
                execution.shown.artifact.content_sha256.clone(),
                execution.shown.artifact.completion_sign_id.as_str().into(),
            ])
        }

        fn initial_routes(
            &self,
            availability: [bool; 2],
        ) -> Result<conduit_std_host::spoken_mask_journey::SpokenMaskRouteSet, String> {
            conduit_std_host::spoken_mask_journey::admit_replacement_spoken_mask_routes(
                &self.initial,
                "orifina-spoken-initial-plan",
                &availability,
                &self.initial_routes.wake,
            )
        }
    }

    impl conduit_presentation::MaskJourneyEmbodiment for HostedSpokenJourney {
        type Outcome = conduit_std_host::spoken_mask_journey::SpokenMaskJourneyObservation;
        type Error = String;

        fn perform(&mut self, action: A) -> Result<Self::Outcome, Self::Error> {
            use conduit_presentation::{MaskShowDisposition, MaskWardrobeAction};
            Ok(match action {
                A::InspectInitialShow => {
                    self.current = Some((false, 0));
                    self.observation(action, "initial ordinary spoken Mask completed Tongues synthesis and an acknowledged artifact Show", self.artifact_receipts()?)
                }
                A::WearAlternateMask => {
                    let revision = self.control.scoped_wardrobe.wardrobe.revision;
                    let evidence = self.control.apply(revision, MaskWardrobeAction::Wear(self.initial[1].mask.form_identity.clone()), &self.initial_routes.routes).map_err(|error| format!("wear hosted spoken Mask: {error:?}"))?;
                    self.observation(action, "alternate ordinary spoken Mask became eligible while the selected route remained current", vec![format!("wardrobe/revision/{}", evidence.resulting_wardrobe.revision)])
                }
                A::PreferAlternateMask => {
                    let routes = self.initial_routes([false, true])?;
                    let revision = self.control.scoped_wardrobe.wardrobe.revision;
                    let evidence = self.control.apply(revision, MaskWardrobeAction::Prefer(vec![self.initial[1].mask.form_identity.clone()]), &routes.routes).map_err(|error| format!("prefer hosted spoken Mask: {error:?}"))?;
                    if !matches!(evidence.reconciliation.show, MaskShowDisposition::SelectSealed { .. }) { return Err("spoken preference did not select the available sealed alternate".into()); }
                    self.initial_routes = routes;
                    self.current = Some((false, 1));
                    self.observation(action, "preference selected the available alternate route under the same immutable Body Plan", vec![format!("wardrobe/revision/{}", evidence.resulting_wardrobe.revision)])
                }
                A::WithdrawSelectedRoute => {
                    let routes = self.initial_routes([false, false])?;
                    let reconciliation = self.control.reconcile_routes(&routes.routes).map_err(|error| format!("withdraw hosted spoken route: {error:?}"))?;
                    if !matches!(reconciliation.show, MaskShowDisposition::NoCurrentShow { .. }) { return Err("spoken route withdrawal retained a current Show".into()); }
                    self.initial_routes = routes;
                    self.current = None;
                    self.observation(action, "the selected route became unavailable and live reconciliation produced NoShow", vec!["route-availability/none".into()])
                }
                A::InspectUnavailableShow => {
                    if self.control.selected.is_some() || self.current.is_some() { return Err("unavailable spoken interval retained a current selection or Show".into()); }
                    self.observation(action, "no current Show; prior WAV artifacts remain stale effect evidence only", vec!["show/current/none".into()])
                }
                A::AddFaceHost => self.observation(action, "replacement Face Host routes became available but remained outside the old immutable Plan", vec![self.replacement_routes.routes.routes()[0].route_id.clone()]),
                A::AdmitReplacementPlan => {
                    let old_plan = self.control.active_plan_id.clone();
                    let reconciliation = self.control.admit_replacement_plan(&old_plan, &self.replacement_routes.body_plan, &self.replacement_routes.routes).map_err(|error| format!("admit hosted spoken replacement Plan: {error:?}"))?;
                    if !matches!(reconciliation.show, MaskShowDisposition::SelectSealed { .. }) { return Err("replacement spoken Plan did not select a sealed route".into()); }
                    self.current = Some((true, 0));
                    self.observation(action, "authorized planning admitted a distinct Body Plan and selected its available spoken route", vec![format!("supersedes/{}", old_plan.as_str())])
                }
                A::InspectReplannedShow => self.observation(action, "replacement ordinary spoken Mask completed Tongues synthesis and an acknowledged artifact Show", self.artifact_receipts()?),
                A::DoffAlternateMask => {
                    let revision = self.control.scoped_wardrobe.wardrobe.revision;
                    let evidence = self.control.apply(revision, MaskWardrobeAction::Doff(self.initial[1].mask.form_identity.clone()), &self.replacement_routes.routes).map_err(|error| format!("doff hosted spoken Mask: {error:?}"))?;
                    if !matches!(evidence.reconciliation.show, MaskShowDisposition::SelectSealed { .. }) { return Err("doffing spoken alternate did not select the restored route".into()); }
                    self.current = Some((true, 1));
                    self.observation(action, "doffing the alternate reconciled the still-worn initial-role Mask to its replacement-Plan route", vec![format!("wardrobe/revision/{}", evidence.resulting_wardrobe.revision)])
                }
                A::InspectRestoredShow => self.observation(action, "restored ordinary spoken Mask completed Tongues synthesis and an acknowledged artifact Show", self.artifact_receipts()?),
            })
        }
    }
    let initial_identity = initial.mask.form_identity.clone();
    let selected = conduit_presentation::SelectedMaskFormRoute {
        route_id: initial_routes.routes.routes()[0].route_id.clone(),
        mask_form: initial_identity.clone(),
        plan_id: initial_routes.body_plan.plan_id.clone(),
    };
    let wardrobe = conduit_presentation::MaskWardrobe::new(
        conduit_presentation::MaskWardrobeLifetime::Body,
        vec![initial_identity],
        vec![],
    )
    .map_err(|error| format!("create hosted spoken wardrobe: {error:?}"))?;
    let control = conduit_presentation::MaskWardrobeControl::new(
        &initial_routes.body_plan.body_id,
        conduit_presentation::BodyMaskWardrobe::new(
            initial_routes.body_plan.body_id.clone(),
            None,
            wardrobe,
        )
        .map_err(|error| format!("scope hosted spoken wardrobe: {error:?}"))?,
        &initial_routes.body_plan,
        &initial_routes.routes,
        Some(selected),
    )
    .map_err(|error| format!("initialize hosted spoken wardrobe control: {error:?}"))?;
    let mut journey_run = HostedSpokenJourney {
        presentation_id: presentation.identity.as_str().into(),
        initial: vec![initial, alternate],
        replacement: vec![replacement_alternate, restored],
        initial_routes,
        replacement_routes,
        control,
        current: None,
    };
    let mut mask_actions = Vec::new();
    conduit_presentation::actualize_mask_journey(&mut journey_run, |_, outcome| {
        mask_actions.push(outcome.clone());
    })
    .map_err(|error| {
        format!(
            "hosted spoken Mask journey failed at {}: {}",
            error.action.id(),
            error.source
        )
    })?;
    let spoken_evidence = conduit_std_host::spoken_mask_journey::SpokenMaskJourneyEvidence {
        schema: conduit_std_host::spoken_mask_journey::SpokenMaskJourneyEvidence::SCHEMA.into(),
        observations: mask_actions.clone(),
        human_hearing_observed: false,
        inward_face_interaction_observed: false,
    };
    spoken_evidence.validate()?;
    let facts = vec![
        serde_json::json!({"initial_body": null, "host_id": journey.receipt.host_ids[0], "boot_id": journey.receipt.boot_ids[0]}),
        serde_json::json!({"provider": manifestation.manifestation.provider_identity, "model": manifestation.manifestation.model_identity}),
        serde_json::json!({"body_id": journey.receipt.body_id, "born_sign_id": born}),
        serde_json::json!({"wake_sign_id": wake, "plan_id": journey.receipt.plan_ids[0]}),
        serde_json::json!({"plan_id": journey.receipt.plan_ids[0], "play_id": journey.receipt.play_ids[0]}),
        serde_json::json!({"face_id": manifestation.source_presentation_identity, "show": manifestation.manifestation}),
        serde_json::json!({"workload_revision": journey.receipt.workload_revision, "workload_sign_id": workload}),
        serde_json::json!({"host_id": journey.receipt.host_ids[1], "boot_id": journey.receipt.boot_ids[1], "membership": evidence.membership}),
        serde_json::json!({"reason": journey.receipt.fault_reason, "fault_sign_id": failed}),
        serde_json::json!({"repaired": journey.receipt.repaired, "plan_id": journey.receipt.plan_ids.last(), "play_id": journey.receipt.play_ids.last()}),
        serde_json::json!({"active_play_id": journey.receipt.play_ids.last(), "presenter_requests": receipt.presenter_requests.len()}),
        serde_json::json!({"lull_sign_id": lulled}),
        serde_json::json!({"fulfilled_sign_id": fulfilled, "fulfilled": journey.receipt.fulfilled}),
    ];
    let (embodiment, mask_form_id) = if receipt.proof_class == "ollama-http-fixture" {
        (
            "hosted-ollama-http-fixture-body",
            "std/ollama-http-fixture@1",
        )
    } else {
        (
            "hosted-open-weight-model-body",
            "std/local-open-weight-model@1",
        )
    };
    body_journey_track::write(
        TrackSource {
            commit: git_head()?,
            track_id: "hosted-generative",
            embodiment,
            mask_form_id,
            construction: journey.receipt.host_ids.iter().map(|host_id| {
                crate::commands::body_journey_track::ConstructionTruth {
                    host_id: host_id.clone(),
                    profile: crate::commands::body_journey_track::ConstructionStage::omitted("This already-running std Host has no target-profile make stage in the hosted journey."),
                    build: crate::commands::body_journey_track::ConstructionStage::omitted("This hosted journey starts admitted implementations directly and produces no standalone build artifact."),
                    image: crate::commands::body_journey_track::ConstructionStage::omitted("This hosted Host is not booted from a journey-produced image, so no image identity is claimed."),
                }
            }).collect(),
            identities: TrackIdentities {
                body: journey.receipt.body_id.clone(),
                host: journey.receipt.host_ids[0].clone(),
                boot: journey.receipt.boot_ids[0].clone(),
                peer_host: journey.receipt.host_ids[1].clone(),
                peer_boot: journey.receipt.boot_ids[1].clone(),
                plan: journey
                    .receipt
                    .plan_ids
                    .last()
                    .cloned()
                    .ok_or("missing Orifina Plan")?,
                distributed_plan: None,
                play: journey
                    .receipt
                    .play_ids
                    .last()
                    .cloned()
                    .ok_or("missing Orifina Play")?,
                face: manifestation.source_presentation_identity.clone(),
                show: manifestation.manifestation.candidate_identity.clone(),
                line: None,
                signs: BTreeMap::from([
                    ("body.born", born),
                    ("body.awake", wake),
                    ("form.used", repaired.clone()),
                    ("workload.revised", workload),
                    ("host.added", "sign/orifina-companion/joined".into()),
                    ("fault.observed", failed),
                    ("body.repaired", repaired.clone()),
                    ("body.long-running", repaired),
                    ("body.lulled", lulled),
                    ("body.fulfilled", fulfilled),
                ]),
            },
            facts,
            mask_actions: serde_json::to_value(mask_actions)
                .map_err(|error| format!("encode hosted Mask journey: {error}"))?,
            action_events: BTreeMap::from([
                ("journey.bootstrap", "The std Host began Orifina's bounded bootstrap before any Body identity existed.".into()),
                ("journey.birth", "The accepted birth and wake receipts created and activated Orifina's independent Body.".into()),
                ("journey.useful-work", "The admitted model Form handled a later Face realization request through the retained Plan and Play.".into()),
                ("journey.break-recover", "A retained model-provider failure was followed by a new admitted repair receipt.".into()),
                ("journey.rest-finish", "Explicit lull and fulfillment signs ended Orifina's wake and biography in order.".into()),
            ]),
        },
        Path::new("target/journeys/three-bodies/hosted-generative"),
    )
    .map_err(std::io::Error::other)?;
    documentary::retain(
        Path::new("target/journeys/three-bodies/hosted-generative"),
        journey,
        receipt,
    )?;
    Ok(())
}

fn git_head() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    if !output.status.success() {
        return Err("cannot resolve exact source commit".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
