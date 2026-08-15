//! Read-only native Parts mode over canonical Body membership truth.

use crate::{gui::GuiAction, PatchbayApplication};
use patchbay_model::PartsView;

impl PatchbayApplication {
    pub(super) fn parts_projection(&self) -> Result<Option<PartsView>, String> {
        if !self.parts_open {
            return Ok(None);
        }
        let body = self
            .build_birth
            .body()
            .ok_or("Parts view requires a born Body")?;
        let membership = self
            .build_birth
            .membership()
            .ok_or("Parts view requires Body membership truth")?;
        let candidates = self
            .body_candidates
            .as_ref()
            .ok_or("Parts view requires candidate inventory truth")?;
        let here = membership
            .parts
            .first()
            .map(|part| &part.part_id)
            .ok_or("Parts view requires the explicit Here Part")?;
        let play = self
            .control
            .is_running()
            .then(|| self.control.planned_play_identity())
            .flatten();
        PartsView::project_with_presence(
            body,
            membership,
            candidates,
            here,
            self.control.plan(),
            play.as_ref(),
            self.build_birth.wake_value().is_some(),
            self.browser_parts
                .as_ref()
                .and_then(super::browser_parts::BrowserPartsCoordinator::presence),
        )
        .map(Some)
        .map_err(|error| format!("Parts projection: {error:?}"))
    }

    pub(super) fn handle_parts_action(&mut self, action: GuiAction) -> Result<(), String> {
        match action {
            GuiAction::TogglePartsView => {
                if self.build_birth.body().is_none() {
                    return Err("Birth a Body before opening Parts".into());
                }
                self.parts_open = !self.parts_open;
                if !self.parts_open {
                    self.selected_part = None;
                    self.selected_candidate = None;
                    self.pending_revoke = None;
                }
            }
            GuiAction::SpawnBrowserPart => {
                let body_id = self
                    .build_birth
                    .body()
                    .ok_or("Birth a Body before spawning a browser Part")?
                    .body_id
                    .clone();
                let target = self
                    .browser_parts
                    .as_mut()
                    .ok_or("Configure the browser page and chat Line before spawning a Part")?
                    .begin(&body_id)?;
                std::process::Command::new("xdg-open")
                    .arg(&target)
                    .spawn()
                    .map_err(|error| format!("cannot open browser Part: {error}"))?;
                self.publish_completed("Browser Part invitation opened; awaiting exact proof");
            }
            GuiAction::CancelBrowserPartSpawn => {
                if !self
                    .browser_parts
                    .as_mut()
                    .is_some_and(super::browser_parts::BrowserPartsCoordinator::cancel)
                {
                    return Err("No browser Part spawn is pending".into());
                }
                self.publish_completed("Browser Part invitation cancelled");
            }
            GuiAction::InspectPart(part_id) => {
                let view = self.parts_projection()?.ok_or("Parts view is not open")?;
                if !view.parts.iter().any(|row| row.details.part_id == part_id) {
                    return Err("selected Part is not in the current Body projection".into());
                }
                self.select_front_door_subject(&format!("part/{}", part_id.as_str()))?;
                self.selected_part = Some(part_id);
                self.selected_candidate = None;
                self.pending_revoke = None;
            }
            GuiAction::InspectCandidate(candidate_id) => {
                let view = self.parts_projection()?.ok_or("Parts view is not open")?;
                if !view
                    .wants_to_join
                    .iter()
                    .any(|row| row.candidate_id == candidate_id)
                {
                    return Err("selected candidate is not in the current Body projection".into());
                }
                self.select_front_door_subject(&format!("candidate/{}", candidate_id.as_str()))?;
                self.selected_candidate = Some(candidate_id);
                self.selected_part = None;
                self.pending_revoke = None;
            }
            GuiAction::AdmitCandidate(candidate_id) => {
                let view = self.parts_projection()?.ok_or("Parts view is not open")?;
                let row = view
                    .wants_to_join
                    .iter()
                    .find(|row| row.candidate_id == candidate_id)
                    .ok_or("candidate is not awaiting a Body decision")?;
                if !row.actions.contains(&patchbay_model::PartsAction::Admit) {
                    return Err("candidate cannot be admitted in its current state".into());
                }
                let mut nonce = [0; 32];
                std::fs::File::open("/dev/urandom")
                    .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut nonce))
                    .map_err(|error| {
                        format!("cannot obtain admission challenge entropy: {error}")
                    })?;
                let requested = self.lifecycle_sign("candidate-admission-requested");
                let now = now_millis()?;
                let is_pico = self
                    .pico_parts
                    .as_ref()
                    .is_some_and(|pico| pico.owns(&candidate_id));
                if is_pico {
                    self.pico_parts
                        .as_mut()
                        .expect("Pico candidate ownership checked")
                        .admit(
                            self.body_candidates
                                .as_mut()
                                .ok_or("Parts view requires candidate inventory truth")?,
                            &candidate_id,
                            nonce,
                            now,
                            requested,
                        )?;
                } else {
                    self.browser_parts
                        .as_mut()
                        .and_then(super::browser_parts::BrowserPartsCoordinator::ambient_mut)
                        .ok_or("candidate has no configured admission transport")?
                        .admit(
                            self.body_candidates
                                .as_mut()
                                .ok_or("Parts view requires candidate inventory truth")?,
                            &candidate_id,
                            nonce,
                            now,
                            requested,
                        )?;
                }
                nonce.fill(0);
                self.publish_completed(format!(
                    "Admission proof requested from candidate {}",
                    candidate_id.as_str()
                ));
            }
            GuiAction::RefuseCandidate(candidate_id) => {
                let view = self.parts_projection()?.ok_or("Parts view is not open")?;
                let row = view
                    .wants_to_join
                    .iter()
                    .find(|row| row.candidate_id == candidate_id)
                    .ok_or("candidate is not awaiting a Body decision")?;
                if !row.actions.contains(&patchbay_model::PartsAction::Refuse) {
                    return Err("candidate cannot be refused in its current state".into());
                }
                if let Some(ambient) = self
                    .browser_parts
                    .as_mut()
                    .and_then(super::browser_parts::BrowserPartsCoordinator::ambient_mut)
                {
                    ambient.refuse(&candidate_id);
                }
                if let Some(pico) = &mut self.pico_parts {
                    pico.refuse(&candidate_id);
                }
                self.lifecycle_sequence = self.lifecycle_sequence.saturating_add(1);
                self.body_candidates
                    .as_mut()
                    .ok_or("Parts view requires candidate inventory truth")?
                    .transition(
                        &candidate_id,
                        conduit_body::CandidateState::Refused,
                        conduit_core::SignId::from(format!(
                            "patchbay-native/candidate-refused/{}",
                            self.lifecycle_sequence
                        )),
                    )
                    .map_err(|refusal| format!("candidate refusal: {refusal:?}"))?;
                self.selected_candidate = None;
                self.publish_refusal(format!(
                    "Candidate {} refused; Body membership is unchanged",
                    candidate_id.as_str()
                ));
            }
            GuiAction::RequestRevokePart(part_id) => {
                let view = self.parts_projection()?.ok_or("Parts view is not open")?;
                let row = view
                    .parts
                    .iter()
                    .find(|row| row.details.part_id == part_id)
                    .ok_or("Part is not in the current Body projection")?;
                if row.state == patchbay_model::PartPresentationState::Here {
                    return Err("the Here Part cannot revoke itself from this Patchbay".into());
                }
                self.pending_revoke = Some(part_id.clone());
                self.selected_part = Some(part_id);
                self.selected_candidate = None;
            }
            GuiAction::ConfirmRevokePart(part_id) => {
                if self.pending_revoke.as_ref() != Some(&part_id) {
                    return Err(
                        "Part revocation requires an exact prior confirmation request".into(),
                    );
                }
                let sign = self.lifecycle_sign("part-revoked");
                self.build_birth
                    .revoke_part(&part_id, sign)
                    .map_err(|error| format!("Part revocation: {error}"))?;
                self.pending_revoke = None;
                self.selected_part = None;
                self.publish_completed(format!("Revoked Part {}", part_id.as_str()));
            }
            _ => return Err("action does not belong to Parts".into()),
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    pub(super) fn poll_body_parts(&mut self) -> Result<bool, String> {
        let presence_update = match (&mut self.browser_parts, &mut self.build_birth) {
            (Some(coordinator), build_birth) => build_birth
                .membership_mut()
                .map(|membership| coordinator.poll_presence(membership))
                .transpose()?
                .flatten(),
            _ => None,
        };
        if let Some(message) = presence_update {
            self.publish_completed(message);
            return Ok(true);
        }
        let pico_disconnect = self
            .pico_parts
            .as_mut()
            .map(super::pico_parts::PicoPartsCoordinator::take_disconnect)
            .transpose()?
            .flatten();
        if let Some((part_id, boot_id)) = pico_disconnect {
            let body_id = self
                .build_birth
                .body()
                .ok_or("Pico disconnect requires a born Body")?
                .body_id
                .clone();
            let offline_sign = self.lifecycle_sign("pico-host-offline");
            let membership = self
                .build_birth
                .membership_mut()
                .ok_or("Pico disconnect requires Body membership")?;
            membership
                .observe_offline(
                    &body_id,
                    membership.revision,
                    &part_id,
                    &boot_id,
                    offline_sign,
                )
                .map_err(|error| format!("mark Pico offline: {error:?}"))?;
            self.publish_completed(format!(
                "Pico Part {} is offline; stale Boot {} cannot be silently rebound",
                part_id.as_str(),
                boot_id.as_str()
            ));
            return Ok(true);
        }
        let pico_candidate = match (&mut self.pico_parts, &mut self.body_candidates) {
            (Some(coordinator), Some(candidates)) => coordinator.poll_candidate(candidates)?,
            _ => None,
        };
        if let Some(candidate_id) = pico_candidate {
            self.publish_completed(format!(
                "Pico candidate {} wants to join; inspect and Admit or Refuse",
                candidate_id.as_str()
            ));
            return Ok(true);
        }
        let pico_proof = self
            .pico_parts
            .as_mut()
            .map(super::pico_parts::PicoPartsCoordinator::take_proof)
            .transpose()?
            .flatten();
        if let Some(arrival) = pico_proof {
            let signs = conduit_body::AdmissionSigns {
                part_admitted: self.lifecycle_sign("pico-part-admitted"),
                host_attached: self.lifecycle_sign("pico-host-attached"),
                candidate_admitted: self.lifecycle_sign("pico-candidate-admitted"),
            };
            let credential = self
                .pico_parts
                .as_mut()
                .expect("Pico proof requires coordinator")
                .complete(
                    arrival,
                    self.body_candidates
                        .as_mut()
                        .ok_or("Pico proof requires candidate inventory")?,
                    self.build_birth
                        .membership_mut()
                        .ok_or("Pico proof arrived before Body membership existed")?,
                    now_millis()?,
                    signs,
                )?;
            self.selected_candidate = None;
            self.publish_completed(format!(
                "Pico Part {} admitted for Host {}",
                credential.part_id.as_str(),
                credential.host_id.as_str()
            ));
            return Ok(true);
        }
        let candidate = match (&mut self.browser_parts, &mut self.body_candidates) {
            (Some(coordinator), Some(candidates)) => coordinator
                .ambient_mut()
                .map(|ambient| ambient.poll_candidate(candidates))
                .transpose()?
                .flatten(),
            _ => None,
        };
        if let Some(candidate_id) = candidate {
            self.publish_completed(format!(
                "Browser candidate {} wants to join; inspect and Admit or Refuse",
                candidate_id.as_str()
            ));
            return Ok(true);
        }
        let ambient_proof = self
            .browser_parts
            .as_mut()
            .and_then(super::browser_parts::BrowserPartsCoordinator::ambient_mut)
            .map(super::browser_ambient::AmbientBrowserCoordinator::take_proof)
            .transpose()?
            .flatten();
        if let Some(arrival) = ambient_proof {
            let signs = conduit_body::AdmissionSigns {
                part_admitted: self.lifecycle_sign("browser-part-admitted"),
                host_attached: self.lifecycle_sign("browser-host-attached"),
                candidate_admitted: self.lifecycle_sign("browser-candidate-admitted"),
            };
            let now = now_millis()?;
            let (browser_parts, candidates, build_birth) = (
                &mut self.browser_parts,
                &mut self.body_candidates,
                &mut self.build_birth,
            );
            let admitted = browser_parts
                .as_mut()
                .and_then(super::browser_parts::BrowserPartsCoordinator::ambient_mut)
                .expect("ambient proof requires coordinator")
                .complete(
                    arrival,
                    candidates
                        .as_mut()
                        .ok_or("ambient proof requires candidate inventory")?,
                    build_birth
                        .membership_mut()
                        .ok_or("ambient proof arrived before Body membership existed")?,
                    now,
                    signs,
                )?;
            let credential = browser_parts
                .as_mut()
                .expect("ambient proof requires browser coordinator")
                .register_ambient_presence(
                    admitted,
                    build_birth
                        .membership_mut()
                        .ok_or("ambient presence requires Body membership")?,
                )?;
            self.selected_candidate = None;
            self.publish_completed(format!(
                "Browser Part {} admitted for Host {}",
                credential.part_id.as_str(),
                credential.host_id.as_str()
            ));
            return Ok(true);
        }
        let arrival = match &mut self.browser_parts {
            Some(coordinator) => coordinator.take_arrival()?,
            None => None,
        };
        let Some(arrival) = arrival else {
            return Ok(false);
        };
        let signs = conduit_body::AdmissionSigns {
            part_admitted: self.lifecycle_sign("browser-part-admitted"),
            host_attached: self.lifecycle_sign("browser-host-attached"),
            candidate_admitted: self.lifecycle_sign("browser-candidate-admitted"),
        };
        let credential = self
            .browser_parts
            .as_mut()
            .expect("arrival requires coordinator")
            .complete(
                arrival,
                self.build_birth
                    .membership_mut()
                    .ok_or("browser Part arrived before Body membership existed")?,
                signs,
            )?;
        self.publish_completed(format!(
            "Browser Part {} admitted for Host {}",
            credential.part_id.as_str(),
            credential.host_id.as_str()
        ));
        Ok(true)
    }
}

fn now_millis() -> Result<u64, String> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock before epoch: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "system clock exceeds Body admission range".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use conduit_body::{CandidateObservation, DiscoveryProofId};
    use conduit_core::{LinkBindingId, SignId};
    use patchbay_model::PatchbayAction;

    #[test]
    fn parts_mode_projects_here_and_selection_without_mutating_body_truth() {
        let directory =
            std::env::temp_dir().join(format!("patchbay-native-parts-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("clock.conduit");
        std::fs::write(&path, include_str!("../../../examples/clock.conduit")).unwrap();
        let mut application = PatchbayApplication::new(Arguments {
            form_path: Some(path.clone()),
            ..Arguments::default()
        })
        .unwrap();

        assert!(application.parts_projection().unwrap().is_none());
        assert!(application
            .handle_parts_action(GuiAction::TogglePartsView)
            .is_err());
        application
            .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Birth))
            .unwrap();
        let before = application.build_birth.membership().unwrap().clone();
        application
            .handle_parts_action(GuiAction::TogglePartsView)
            .unwrap();
        let view = application.parts_projection().unwrap().unwrap();
        assert_eq!(view.parts.len(), 1);
        assert_eq!(view.parts[0].label, "This computer");
        assert_eq!(
            view.parts[0].state,
            patchbay_model::PartPresentationState::Here
        );
        assert!(view.parts[0].available);
        let part_id = view.parts[0].details.part_id.clone();
        let mut pixels = vec![crate::BACKGROUND; 1_100 * 720];
        let mut lifecycle = crate::gui::LifecycleContext {
            body_id: Some(view.body_id.as_str().into()),
            parts: Some(view.clone()),
            ..Default::default()
        };
        let targets = crate::gui::draw_patchbay(
            &mut pixels,
            1_100,
            720,
            application.graphical_form.as_ref().unwrap(),
            crate::gui::PatchbayViewContext {
                selected: None,
                breadcrumb: "",
                lifecycle: &lifecycle,
                palette: &Default::default(),
                forms: &[],
                form_selection: 0,
                form_scroll: 0,
                exact_identity_open: false,
                face_control_focus: 0,
                presentation_layout: &application.layout,
                realization_plan: None,
                realization_hosts: &[],
                status: None,
                gesture: Default::default(),
                viewport: &Default::default(),
            },
        );
        assert!(targets.iter().any(
            |target| matches!(&target.action, GuiAction::InspectPart(candidate) if candidate == &part_id)
        ));
        assert!(targets
            .iter()
            .any(|target| target.action == GuiAction::SpawnBrowserPart));
        lifecycle.browser_spawn_pending = true;
        let cancel_targets = crate::gui::draw_patchbay(
            &mut pixels,
            1_100,
            720,
            application.graphical_form.as_ref().unwrap(),
            crate::gui::PatchbayViewContext {
                selected: None,
                breadcrumb: "",
                lifecycle: &lifecycle,
                palette: &Default::default(),
                forms: &[],
                form_selection: 0,
                form_scroll: 0,
                exact_identity_open: false,
                face_control_focus: 0,
                presentation_layout: &application.layout,
                realization_plan: None,
                realization_hosts: &[],
                status: None,
                gesture: Default::default(),
                viewport: &Default::default(),
            },
        );
        assert!(cancel_targets
            .iter()
            .any(|target| target.action == GuiAction::CancelBrowserPartSpawn));
        assert!(pixels.contains(&patchbay_model::PHOSPHOR_THEME.focus.packed_rgb()));
        application
            .handle_parts_action(GuiAction::InspectPart(part_id.clone()))
            .unwrap();
        assert_eq!(application.selected_part, Some(part_id));
        assert_eq!(application.build_birth.membership(), Some(&before));

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn f12_uses_the_same_typed_toggle_as_pointer_activation() {
        let mut application = PatchbayApplication::new(Arguments::default()).unwrap();
        assert_eq!(
            application
                .handle_parts_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F12)),
            Err("Birth a Body before opening Parts".into())
        );
        assert_eq!(
            application.handle_parts_key(&winit::keyboard::Key::Character("p".into())),
            Ok(false)
        );
    }

    #[test]
    fn explicit_refuse_records_canonical_state_without_changing_membership() {
        let directory = std::env::temp_dir().join(format!(
            "patchbay-native-parts-refuse-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("clock.conduit");
        std::fs::write(&path, include_str!("../../../examples/clock.conduit")).unwrap();
        let mut application = PatchbayApplication::new(Arguments {
            form_path: Some(path.clone()),
            ..Arguments::default()
        })
        .unwrap();
        application
            .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Birth))
            .unwrap();
        application
            .handle_parts_action(GuiAction::TogglePartsView)
            .unwrap();
        let membership_before = application.build_birth.membership().unwrap().clone();
        let mut advertisement = application.model.advertisement().clone();
        advertisement.host_id = conduit_core::HostId::from("browser/refused");
        advertisement.boot_id = conduit_core::BootId::from("browser/refused-boot");
        let candidate = application
            .body_candidates
            .as_mut()
            .unwrap()
            .observe(CandidateObservation {
                advertisement,
                friendly_label: "Browser refusal candidate".into(),
                observed_binding_id: LinkBindingId::from("line/browser-refused"),
                observation_sign_id: SignId::from("sign/browser-refused-observed"),
                proof_id: DiscoveryProofId::bind("proof/browser-refused-discovery").unwrap(),
                freshness_sequence: 1,
                encoded_bytes: 512,
            })
            .unwrap();

        application
            .handle_parts_action(GuiAction::InspectCandidate(candidate.clone()))
            .unwrap();
        let view = application.parts_projection().unwrap().unwrap();
        let mut pixels = vec![crate::BACKGROUND; 1_100 * 720];
        let lifecycle = crate::gui::LifecycleContext {
            body_id: Some(view.body_id.as_str().into()),
            parts: Some(view),
            selected_candidate: Some(candidate.clone()),
            ..Default::default()
        };
        let targets = crate::gui::draw_patchbay(
            &mut pixels,
            1_100,
            720,
            application.graphical_form.as_ref().unwrap(),
            crate::gui::PatchbayViewContext {
                selected: None,
                breadcrumb: "",
                lifecycle: &lifecycle,
                palette: &Default::default(),
                forms: &[],
                form_selection: 0,
                form_scroll: 0,
                exact_identity_open: false,
                face_control_focus: 0,
                presentation_layout: &application.layout,
                realization_plan: None,
                realization_hosts: &[],
                status: None,
                gesture: Default::default(),
                viewport: &Default::default(),
            },
        );
        assert!(targets.iter().any(
            |target| matches!(&target.action, GuiAction::RefuseCandidate(id) if id == &candidate)
        ));
        assert!(targets.iter().any(
            |target| matches!(&target.action, GuiAction::AdmitCandidate(id) if id == &candidate)
        ));
        assert_eq!(
            application.handle_parts_action(GuiAction::AdmitCandidate(candidate.clone())),
            Err("candidate has no configured admission transport".into())
        );
        application
            .handle_parts_action(GuiAction::RefuseCandidate(candidate.clone()))
            .unwrap();

        assert_eq!(
            application.build_birth.membership(),
            Some(&membership_before)
        );
        assert_eq!(
            application.body_candidates.as_ref().unwrap().candidates[0].state,
            conduit_body::CandidateState::Refused
        );
        assert!(application
            .parts_projection()
            .unwrap()
            .unwrap()
            .wants_to_join
            .is_empty());
        assert!(application.selected_candidate.is_none());

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn revocation_requires_two_exact_actions_and_cannot_target_here() {
        let directory = std::env::temp_dir().join(format!(
            "patchbay-native-parts-revoke-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("clock.conduit");
        std::fs::write(&path, include_str!("../../../examples/clock.conduit")).unwrap();
        let mut application = PatchbayApplication::new(Arguments {
            form_path: Some(path.clone()),
            ..Arguments::default()
        })
        .unwrap();
        application.birth_body().unwrap();
        application.parts_open = true;
        let body_id = application.build_birth.body().unwrap().body_id.clone();
        let here = application.build_birth.membership().unwrap().parts[0]
            .part_id
            .clone();
        assert!(application
            .handle_parts_action(GuiAction::RequestRevokePart(here))
            .is_err());
        let remote = conduit_body::PartId::bind(&body_id, "browser/revoked", 2).unwrap();
        let membership = application.build_birth.membership_mut().unwrap();
        membership
            .admit(
                &body_id,
                membership.revision,
                remote.clone(),
                conduit_body::MembershipProofId::bind("proof/browser/revoked").unwrap(),
                SignId::from("sign/browser/revoked/admitted"),
            )
            .unwrap();
        assert!(application
            .handle_parts_action(GuiAction::ConfirmRevokePart(remote.clone()))
            .is_err());
        application
            .handle_parts_action(GuiAction::RequestRevokePart(remote.clone()))
            .unwrap();
        assert_eq!(application.pending_revoke, Some(remote.clone()));
        application
            .handle_parts_action(GuiAction::ConfirmRevokePart(remote.clone()))
            .unwrap();
        assert_eq!(
            application
                .build_birth
                .membership()
                .unwrap()
                .parts
                .iter()
                .find(|part| part.part_id == remote)
                .unwrap()
                .state,
            conduit_body::MembershipState::Revoked
        );
        assert!(application.pending_revoke.is_none());

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
