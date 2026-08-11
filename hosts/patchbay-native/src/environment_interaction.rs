//! Native interactions over the single bounded authored-environment model.

use crate::{environment_resource::save_environment_resource, gui::GuiAction, PatchbayApplication};
use patchbay_model::{AuthoredLink, AuthoredPart, MachineProfile};
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn handle_environment_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.environment.is_none() {
            return Ok(false);
        }
        if matches!(key, Key::Named(NamedKey::Enter) | Key::Named(NamedKey::F2)) {
            self.environment_name_editing = !self.environment_name_editing;
            return Ok(true);
        }
        if !self.environment_name_editing {
            return Ok(false);
        }
        let part_id = self
            .selected_environment_part
            .clone()
            .ok_or("select a physical part before editing its name")?;
        let environment = self
            .environment
            .as_mut()
            .ok_or("authored environment is absent")?;
        let current = environment
            .parts
            .iter()
            .find(|part| part.part_id == part_id)
            .ok_or("selected physical part is stale")?
            .name
            .clone();
        let next = match key {
            Key::Named(NamedKey::Backspace) => {
                let mut value = current;
                value.pop();
                value
            }
            Key::Character(value) if !self.modifiers.control_key() && !self.modifiers.alt_key() => {
                format!("{current}{value}")
            }
            _ => return Ok(false),
        };
        if next.is_empty() {
            return Ok(true);
        }
        environment
            .rename_part(&part_id, next)
            .map_err(|error| format!("environment rename: {error:?}"))?;
        Ok(true)
    }

    pub(super) fn handle_environment_action(&mut self, action: GuiAction) -> Result<(), String> {
        match action {
            GuiAction::EnvironmentAdd(profile) => self.add_environment_part(profile)?,
            GuiAction::EnvironmentSelect(part_id) => {
                self.environment_name_editing = false;
                if let Some((left, kind)) = self.pending_environment_link.take() {
                    if left != part_id {
                        let environment = self
                            .environment
                            .as_mut()
                            .ok_or("authored environment is absent")?;
                        environment
                            .add_link(AuthoredLink {
                                link_id: format!("{}-{:?}-{}", left, kind, part_id)
                                    .to_ascii_lowercase(),
                                left_part_id: left,
                                right_part_id: part_id.clone(),
                                kind,
                            })
                            .map_err(|error| format!("environment link: {error:?}"))?;
                    }
                }
                self.selected_environment_part = Some(part_id);
            }
            GuiAction::EnvironmentRemove(part_id) => {
                self.environment
                    .as_mut()
                    .ok_or("authored environment is absent")?
                    .remove_part(&part_id)
                    .map_err(|error| format!("environment remove: {error:?}"))?;
                self.selected_environment_part = None;
                self.pending_environment_link = None;
            }
            GuiAction::EnvironmentSave => {
                save_environment_resource(
                    self.environment_path
                        .as_deref()
                        .ok_or("environment path is absent")?,
                    self.environment
                        .as_ref()
                        .ok_or("authored environment is absent")?,
                )?;
            }
            GuiAction::EnvironmentLink(kind) => {
                let part = self
                    .selected_environment_part
                    .clone()
                    .ok_or("select the first physical part before linking")?;
                self.pending_environment_link = Some((part, kind));
            }
            _ => {
                return Err(
                    "Form action is unavailable in the authored-environment workspace".into(),
                )
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    fn add_environment_part(&mut self, profile: MachineProfile) -> Result<(), String> {
        let environment = self
            .environment
            .as_mut()
            .ok_or("authored environment is absent")?;
        let sequence = environment.parts.len() + 1;
        let prefix = match profile {
            MachineProfile::PicoW => "pico",
            MachineProfile::RaspberryPi5 => "rpi",
            MachineProfile::LaptopLinux => "laptop",
        };
        let id = format!("{prefix}-{sequence}");
        let mut part = AuthoredPart::reviewed(&id, profile.human_name(), profile);
        part.x = 220 + (sequence as i32 - 1) * 210;
        part.y = 150;
        environment
            .add_part(part)
            .map_err(|error| format!("environment add: {error:?}"))?;
        self.selected_environment_part = Some(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arguments::Arguments, environment_resource::open_environment_resource};

    #[test]
    fn native_actions_edit_save_and_reopen_one_simulation_only_document() {
        let path = std::env::temp_dir().join(format!("maker-actions-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut application = PatchbayApplication::new(Arguments {
            environment_path: Some(path.clone()),
            ..Default::default()
        })
        .unwrap();
        application
            .handle_environment_action(GuiAction::EnvironmentAdd(MachineProfile::PicoW))
            .unwrap();
        let pico = application.selected_environment_part.clone().unwrap();
        application
            .handle_environment_key(&Key::Named(NamedKey::F2))
            .unwrap();
        application
            .handle_environment_key(&Key::Character(" workshop".into()))
            .unwrap();
        application
            .handle_environment_key(&Key::Named(NamedKey::Enter))
            .unwrap();
        application
            .handle_environment_action(GuiAction::EnvironmentLink(
                patchbay_model::EnvironmentLinkKind::Wifi,
            ))
            .unwrap();
        application
            .handle_environment_action(GuiAction::EnvironmentAdd(MachineProfile::LaptopLinux))
            .unwrap();
        let laptop = application.selected_environment_part.clone().unwrap();
        application
            .handle_environment_action(GuiAction::EnvironmentSelect(laptop))
            .unwrap();
        application
            .environment
            .as_mut()
            .unwrap()
            .move_part(&pico, 420, 260)
            .unwrap();
        application
            .handle_environment_action(GuiAction::EnvironmentSave)
            .unwrap();

        let reopened = open_environment_resource(&path).unwrap();
        assert_eq!(reopened.parts.len(), 2);
        assert_eq!(reopened.links.len(), 1);
        assert!(reopened.parts[0].name.ends_with(" workshop"));
        assert_eq!((reopened.parts[0].x, reopened.parts[0].y), (420, 260));
        let projection = reopened.simulation_projection().unwrap();
        assert!(!projection.provenance.observed_live_truth);
        assert!(!projection.provenance.physical_evidence);
        assert!(!projection.provenance.authority_granted);
        std::fs::remove_file(path).unwrap();
    }
}
