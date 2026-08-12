use super::*;

pub(super) enum LifecycleActionError {
    Unavailable,
    Failure(String),
}

impl PatchbayApplication {
    pub(super) fn lifecycle_sign(&mut self, label: &str) -> SignId {
        let sign = SignId::from(format!(
            "patchbay-native/{label}/{}",
            self.lifecycle_sequence
        ));
        self.lifecycle_sequence = self.lifecycle_sequence.saturating_add(1);
        sign
    }

    pub(super) fn birth_body(&mut self) -> Result<(), String> {
        let sign = self.lifecycle_sign("born");
        let sequence = self.lifecycle_sequence;
        self.build_birth
            .birth(
                self.form_editor
                    .as_ref()
                    .ok_or("Birth requires BUILD mode with a Form")?,
                sequence,
                sign,
            )
            .map_err(|error| error.to_string())
    }

    pub(super) fn wake_body(&mut self) -> Result<(), String> {
        let sign = self.lifecycle_sign("woke");
        let sequence = self.lifecycle_sequence;
        self.build_birth
            .wake(sequence, sign)
            .map_err(|error| error.to_string())
    }

    pub(super) fn plan_play(&mut self) -> Result<(), String> {
        self.plan_play_classified().map_err(|error| match error {
            LifecycleActionError::Unavailable => {
                "planning is unavailable for the current exact Form and Host offers".into()
            }
            LifecycleActionError::Failure(error) => error,
        })
    }

    pub(super) fn plan_play_classified(&mut self) -> Result<(), LifecycleActionError> {
        let editor = self.form_editor.as_ref().ok_or_else(|| {
            LifecycleActionError::Failure("planning requires BUILD mode with a Form".into())
        })?;
        self.control
            .request_plan(editor)
            .map_err(|_| LifecycleActionError::Unavailable)?;
        let plan = self.control.plan().cloned().ok_or_else(|| {
            LifecycleActionError::Failure("planner accepted no exact Plan".into())
        })?;
        let sign = self.lifecycle_sign("planned");
        self.build_birth
            .plan_ready(&plan, sign)
            .map_err(|error| LifecycleActionError::Failure(error.to_string()))
    }

    pub(super) fn play_plan(&mut self) -> Result<(), String> {
        self.play_plan_classified().map_err(|error| match error {
            LifecycleActionError::Unavailable => {
                "Play is unavailable for the current exact Plan and Host offers".into()
            }
            LifecycleActionError::Failure(error) => error,
        })
    }

    pub(super) fn play_plan_classified(&mut self) -> Result<(), LifecycleActionError> {
        let play = self
            .control
            .planned_play_identity()
            .ok_or(LifecycleActionError::Unavailable)?;
        let sign = self.lifecycle_sign("played");
        let mut next = self.build_birth.clone();
        next.play_started(&play, sign)
            .map_err(|error| LifecycleActionError::Failure(error.to_string()))?;
        let editor = self.form_editor.as_ref().ok_or_else(|| {
            LifecycleActionError::Failure("Play requires BUILD mode with a Form".into())
        })?;
        self.control
            .run(editor)
            .map_err(|_| LifecycleActionError::Unavailable)?;
        self.build_birth = next;
        Ok(())
    }

    pub(super) fn mark_unsatisfied(&mut self) -> Result<(), String> {
        let plan_id = self
            .control
            .plan()
            .map(|plan| plan.plan_id.clone())
            .ok_or("unsatisfied transition requires a current Plan")?;
        let sign = self.lifecycle_sign("unsatisfied");
        self.build_birth
            .became_unsatisfied(&plan_id, sign)
            .map_err(|error| error.to_string())
    }

    pub(super) fn lull_body(&mut self) -> Result<(), String> {
        if self.control.is_running() {
            return Err("Lull is distinct from stopping an active Play; stop it first".into());
        }
        let lulled = self.lifecycle_sign("lulled");
        let retained = self.lifecycle_sign("lull-retained");
        self.build_birth
            .lull(lulled, retained)
            .map_err(|error| error.to_string())
    }
}
