//! Bounded physical inputs; no application or renderer state reconstruction.
use super::*;
impl Run<'_> {
    pub(super) fn capture(&mut self, name: &str) -> Result<(), ConduitosError> {
        // Keep one failure-capture slot within the existing27, no limit increase.
        if self.captures >= 26 {
            return Err(refusal("control page captures exceed existing journey cap"));
        }
        self.artifacts
            .capture(self.stream, self.reader, name, false)?;
        self.captures += 1;
        Ok(())
    }
    pub(super) fn press(&mut self, keys: &[&str]) -> Result<Value, ConduitosError> {
        let before = rows(&text(self.serial)?, FOCUS)?.len();
        journey_input::held_key(
            |down| {
                hid_qmp::send_named_keys(
                    self.stream,
                    self.reader,
                    keys,
                    down,
                    "creche-controls-input",
                )
            },
            || {
                journey_input::wait_for_record_retaining_guest(
                    self.serial,
                    self.child,
                    "creche-controls-input-timeout",
                    "focus",
                    |s| Ok(rows(s, FOCUS)?.len() > before),
                )
            },
        )?;
        let s = snapshot(&text(self.serial)?)?;
        self.observations.push(json!({"keys":keys,"focus":s.focus}));
        Ok(s.focus)
    }
    pub(super) fn traverse(
        &mut self,
        initial: &Snapshot,
        expected: &[(String, Option<String>)],
        reverse: bool,
    ) -> Result<Vec<Value>, ConduitosError> {
        let mut seen = BTreeSet::new();
        let mut records = Vec::new();
        for step in 0..=expected.len() {
            let current = snapshot(&text(self.serial)?)?;
            unchanged(initial, &current)?;
            seen.insert(key(&current.focus)?);
            records.push(current.focus);
            if step < expected.len() {
                self.press(if reverse { &["shift", "tab"] } else { &["tab"] })?;
            }
        }
        if seen != expected.iter().cloned().collect() {
            return Err(refusal(
                "observed focus inventory differs from actual available Source controls",
            ));
        }
        if key(records.first().unwrap())? != key(records.last().unwrap())? {
            return Err(refusal(
                "bounded focus traversal did not close one exact cycle",
            ));
        }
        Ok(records)
    }
    pub(super) fn pages(
        &mut self,
        initial: &Snapshot,
        label: &str,
    ) -> Result<Vec<Value>, ConduitosError> {
        let start = snapshot(&text(self.serial)?)?;
        unchanged(initial, &start)?;
        let count = start.focus["pages"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 24)
            .ok_or_else(|| refusal("page count outside finite capture profile"))?;
        // PageUp is clamped; each real report is observed even at an endpoint.
        for _ in 0..count {
            self.press(&["pgup"])?;
        }
        let mut pages = Vec::new();
        for index in 1..=count {
            let current = snapshot(&text(self.serial)?)?;
            unchanged(initial, &current)?;
            if current.focus["page"] != index || current.focus["pages"] != count {
                return Err(refusal("page traversal lost exact endpoint/count"));
            }
            self.capture(&format!("creche-controls-{label}-{index}"))?;
            pages.push(current.focus);
            if index < count {
                self.press(&["pgdn"])?;
            }
        }
        let end = self.press(&["pgdn"])?;
        if end["page"] != count {
            return Err(refusal("PageDown endpoint failed to clamp"));
        }
        Ok(pages)
    }
    pub(super) fn commit(&mut self, before: usize) -> Result<(), ConduitosError> {
        journey_input::held_key(
            |down| {
                hid_qmp::send_named_keys(
                    self.stream,
                    self.reader,
                    &["ret"],
                    down,
                    "creche-controls-commit",
                )
            },
            || {
                journey_input::wait_for_record_retaining_guest(
                    self.serial,
                    self.child,
                    "creche-controls-change-timeout",
                    "checked-change",
                    |s| {
                        let changes = rows(s, CHANGE)?;
                        let faces = rows(s, "CONDUIT_NATIVE_FACE ")?;
                        let focuses = rows(s, FOCUS)?;
                        Ok(changes.len() > before
                            && changes
                                .last()
                                .zip(faces.last())
                                .zip(focuses.last())
                                .is_some_and(|((c, f), focus)| {
                                    c["after_face_id"] == f["face"]["identity"]
                                        && c["after_show_id"] == f["show"]["show_id"]
                                        && focus["face_id"] == c["after_face_id"]
                                        && focus["show_id"] == c["after_show_id"]
                                }))
                    },
                )
            },
        )
    }
    pub(super) fn rename(&mut self, name: &[u8], capture: &str) -> Result<Value, ConduitosError> {
        let spelling = keys_for(name)?;
        creche_choices::navigate_to(
            self.serial,
            self.child,
            self.stream,
            self.reader,
            "creche.name",
            Some("value"),
            false,
        )?;
        let before = snapshot(&text(self.serial)?)?;
        let count = rows(&text(self.serial)?, CHANGE)?.len();
        for keys in &spelling {
            let borrowed = keys.iter().map(String::as_str).collect::<Vec<_>>();
            self.press(&borrowed)?;
        }
        self.commit(count)?;
        let change = validate_change(&text(self.serial)?, &before, "creche.name", name, count)?;
        self.capture(capture)?;
        Ok(change)
    }
    pub(super) fn naming(
        &mut self,
        members: &[Vec<u8>],
        desired: &[u8],
        capture: &str,
    ) -> Result<(), ConduitosError> {
        creche_choices::navigate_to(
            self.serial,
            self.child,
            self.stream,
            self.reader,
            "creche.naming",
            Some("value"),
            false,
        )?;
        let before = snapshot(&text(self.serial)?)?;
        let old = current(&before.face, "creche.naming")?;
        let at = members
            .iter()
            .position(|v| v == &old)
            .ok_or_else(|| refusal("current naming value outside membership"))?;
        let target = members
            .iter()
            .position(|v| v == desired)
            .ok_or_else(|| refusal("desired naming outside membership"))?;
        let steps = (target + members.len() - at) % members.len();
        // If original already Welsh, one full cycle still performs a real checked commit.
        for _ in 0..if steps == 0 { members.len() } else { steps } {
            self.press(&["right"])?;
            unchanged(&before, &snapshot(&text(self.serial)?)?)?;
        }
        let count = rows(&text(self.serial)?, CHANGE)?.len();
        self.commit(count)?;
        let change = validate_change(
            &text(self.serial)?,
            &before,
            "creche.naming",
            desired,
            count,
        )?;
        let after = snapshot(&text(self.serial)?)?;
        if retained_selection(&before.face) != retained_selection(&after.face) {
            return Err(refusal("naming changed unrelated search/Plot selection"));
        }
        self.observations.push(json!({"checked_change":change}));
        self.capture(capture)
    }
}
