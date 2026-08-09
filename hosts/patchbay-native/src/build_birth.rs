use super::*;

impl PatchbayApplication {
    pub(super) fn lifecycle_clue(&mut self, label: &str) -> ClueId {
        let clue = ClueId::from(format!(
            "patchbay-native/{label}/{}",
            self.lifecycle_sequence
        ));
        self.lifecycle_sequence = self.lifecycle_sequence.saturating_add(1);
        clue
    }

    pub(super) fn birth_body(&mut self) -> Result<(), String> {
        let clue = self.lifecycle_clue("born");
        let sequence = self.lifecycle_sequence;
        self.build_birth
            .birth(
                self.form_editor
                    .as_ref()
                    .ok_or("Birth requires BUILD mode with a Form")?,
                sequence,
                clue,
            )
            .map_err(|error| error.to_string())
    }

    pub(super) fn wake_body(&mut self) -> Result<(), String> {
        let clue = self.lifecycle_clue("woke");
        let sequence = self.lifecycle_sequence;
        self.build_birth
            .wake(sequence, clue)
            .map_err(|error| error.to_string())
    }

    pub(super) fn plan_play(&mut self) -> Result<(), String> {
        let editor = self
            .form_editor
            .as_ref()
            .ok_or("planning requires BUILD mode with a Form")?;
        self.control.request_plan(editor)?;
        let plan = self
            .control
            .plan()
            .cloned()
            .ok_or("planner accepted no exact Plan")?;
        let clue = self.lifecycle_clue("planned");
        self.build_birth
            .plan_ready(&plan, clue)
            .map_err(|error| error.to_string())
    }

    pub(super) fn play_plan(&mut self) -> Result<(), String> {
        let play = self
            .control
            .planned_play_identity()
            .ok_or("Play requires a current exact Plan")?;
        let clue = self.lifecycle_clue("played");
        let mut next = self.build_birth.clone();
        next.play_started(&play, clue)
            .map_err(|error| error.to_string())?;
        self.control.run(
            self.form_editor
                .as_ref()
                .ok_or("Play requires BUILD mode with a Form")?,
        )?;
        self.build_birth = next;
        Ok(())
    }

    pub(super) fn mark_unsatisfied(&mut self) -> Result<(), String> {
        let plan_id = self
            .control
            .plan()
            .map(|plan| plan.plan_id.clone())
            .ok_or("unsatisfied transition requires a current Plan")?;
        let clue = self.lifecycle_clue("unsatisfied");
        self.build_birth
            .became_unsatisfied(&plan_id, clue)
            .map_err(|error| error.to_string())
    }

    pub(super) fn lull_body(&mut self) -> Result<(), String> {
        if self.control.is_running() {
            return Err("Lull is distinct from stopping an active Play; stop it first".into());
        }
        let lulled = self.lifecycle_clue("lulled");
        let retained = self.lifecycle_clue("lull-retained");
        self.build_birth
            .lull(lulled, retained)
            .map_err(|error| error.to_string())
    }
}
