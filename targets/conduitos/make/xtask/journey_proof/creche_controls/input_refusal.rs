//! Natural incomplete text-field submission; no forged interaction or replay.
use super::*;
const REFUSAL: &str = "CONDUIT_CRECHE_INPUT_REFUSAL ";
const CODE: &str = "native-face-mask-input-incomplete";
impl Run<'_> {
    pub(super) fn incomplete_argument_refusal(&mut self) -> Result<Value, ConduitosError> {
        creche_choices::navigate_to(
            self.serial,
            self.child,
            self.stream,
            self.reader,
            "creche.name",
            Some("value"),
            false,
        )?;
        let before_text = text(self.serial)?;
        let before = snapshot(&before_text)?;
        let signs = rows(&before_text, REFUSAL)?.len();
        let changes = rows(&before_text, CHANGE)?.len();
        let publications = rows(&before_text, "CONDUIT_NATIVE_FACE ")?.len();
        journey_input::held_key(
            |down| {
                hid_qmp::send_named_keys(
                    self.stream,
                    self.reader,
                    &["ret"],
                    down,
                    "creche-incomplete-field",
                )
            },
            || {
                journey_input::wait_for_record_retaining_guest(
                    self.serial,
                    self.child,
                    "creche-incomplete-refusal-timeout",
                    "incomplete-field",
                    |observed| {
                        let rows = rows(observed, REFUSAL)?;
                        Ok(rows.len() > signs
                            && rows.last().is_some_and(|row| {
                                row["status"] == "refused" && row["reason"] == CODE
                            }))
                    },
                )
            },
        )?;
        let after_text = text(self.serial)?;
        let after = snapshot(&after_text)?;
        unchanged(&before, &after)?;
        let observed = rows(&after_text, REFUSAL)?;
        if observed.len() != signs + 1
            || rows(&after_text, CHANGE)?.len() != changes
            || rows(&after_text, "CONDUIT_NATIVE_FACE ")?.len() != publications
        {
            return Err(refusal(
                "incomplete field changed Source or emitted extra refusal/publication",
            ));
        }
        let observed_refusal = observed.last().unwrap();
        if observed_refusal["schema"] != "conduit.creche-input-refusal/v1"
            || observed_refusal["notice"] != "Enter value"
            || observed_refusal["scanout_acknowledged"] != true
            || observed_refusal["source_owner_called"] != false
            || observed_refusal["face_id"] != before.face.identity.as_str()
            || observed_refusal["face_revision"] != before.face.revision
            || observed_refusal["show_id"] != before.record["show"]["show_id"]
        {
            return Err(refusal(
                "incomplete refusal lacks actual unchanged Show/scanout custody",
            ));
        }
        self.capture("creche-input-incomplete")?;
        Ok(json!({"schema":"conduit.conduitos/creche-input-refusal@1",
            "input":"one actual Enter press on the focused unedited name argument",
            "refusal":observed.last().unwrap(),"source_changes":0,"new_publications":0,
            "before":before.record,"after":after.record,"focus":after.focus,
            "capture":"creche-input-incomplete.png",
            "visibility":"original QMP capture; visual inspection required, no OCR inference",
            "recovery":"subsequent checked probe rename in parent receipt"}))
    }
}
