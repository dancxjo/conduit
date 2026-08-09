use conduit_core::{bind_active_play, bind_presentation, bind_sign};
use conduit_signal::triple;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PicoRuntimeIdentity {
    pub boot_id: String,
    pub active_play_id: String,
}

pub struct PicoSign {
    plan_id: String,
    fragment_id: String,
    host_id: String,
    image_boot_id: String,
    image_active_play_id: String,
    placement_id: conduit_core::PlacementId,
    firmware_build_id: Option<String>,
    source_document_id: Option<String>,
    checked_form_id: Option<String>,
    expanded_form_id: Option<String>,
}

impl PicoSign {
    pub fn exact_triple() -> Result<Self, String> {
        let exact = triple::exact_plan()?;
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.pico_advertisement.host_id)
            .ok_or_else(|| "triple Pico fragment missing".to_owned())?;
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.gear_id.as_str() == "light")
            .ok_or_else(|| "triple Pico light placement missing".to_owned())?;
        let image_active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        Ok(Self {
            plan_id: fragment.plan_id.as_str().to_owned(),
            fragment_id: fragment.fragment_id.as_str().to_owned(),
            host_id: fragment.host_id.as_str().to_owned(),
            image_boot_id: fragment.boot_id.as_str().to_owned(),
            image_active_play_id: image_active_play.active_play_id.as_str().to_owned(),
            placement_id: placement.placement_id.clone(),
            firmware_build_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
        })
    }

    pub fn verify_boot(&mut self, line: &str) -> Result<PicoRuntimeIdentity, String> {
        let record = parse(line, "conduit-pico-w-signal/boot@1")?;
        self.verify_plan_fields(&record, false)?;
        let expected_boot_sign = bind_sign(
            &conduit_core::HostId::from(self.host_id.as_str()),
            &conduit_core::BootId::from(self.image_boot_id.as_str()),
            None,
            0,
        );
        field(&record, "sign_id", expected_boot_sign.sign_id.as_str())?;
        self.capture_build_fields(&record)?;
        let boot_id = string(&record, "runtime_boot_id")?.to_owned();
        let active_play_id = string(&record, "runtime_active_play_id")?.to_owned();
        if boot_id == self.image_boot_id || active_play_id == self.image_active_play_id {
            return Err("Pico runtime reused a generated-image identity".to_owned());
        }
        let expected = bind_active_play(
            &conduit_core::PlanId::from(self.plan_id.as_str()),
            &conduit_core::HostId::from(self.host_id.as_str()),
            &conduit_core::BootId::from(boot_id.as_str()),
            0,
        );
        if active_play_id != expected.active_play_id.as_str() {
            return Err("Pico runtime play is not canonically boot-bound".to_owned());
        }
        Ok(PicoRuntimeIdentity {
            boot_id,
            active_play_id,
        })
    }

    pub fn verify_receipt(
        &self,
        line: &str,
        runtime: &PicoRuntimeIdentity,
        sequence: u64,
        level: bool,
    ) -> Result<(), String> {
        let record = parse(line, "conduit-pico-w-signal/receipt@1")?;
        self.verify_plan_fields(&record, true)?;
        self.verify_build_fields(&record)?;
        verify_runtime(&record, runtime)?;
        if record["sequence"].as_u64() != Some(sequence) || record["level"].as_bool() != Some(level)
        {
            return Err(format!("Pico receipt value mismatch: {line}"));
        }
        let presentation = bind_presentation(
            &conduit_core::ActivePlayId::from(self.image_active_play_id.as_str()),
            &self.placement_id,
            sequence,
        );
        let sign = bind_sign(
            &conduit_core::HostId::from(self.host_id.as_str()),
            &conduit_core::BootId::from(self.image_boot_id.as_str()),
            Some(&conduit_core::ActivePlayId::from(
                self.image_active_play_id.as_str(),
            )),
            sequence,
        );
        field(
            &record,
            "presentation_id",
            presentation.presentation_id.as_str(),
        )?;
        field(&record, "sign_id", sign.sign_id.as_str())
    }

    pub fn verify_terminal(
        &self,
        line: &str,
        runtime: &PicoRuntimeIdentity,
        success: bool,
    ) -> Result<(), String> {
        let record = parse(line, "conduit-pico-w-signal/terminal@1")?;
        self.verify_plan_fields(&record, true)?;
        self.verify_build_fields(&record)?;
        verify_runtime(&record, runtime)?;
        if record["success"].as_bool() != Some(success) {
            return Err(format!("Pico terminal disposition mismatch: {line}"));
        }
        let sign = bind_sign(
            &conduit_core::HostId::from(self.host_id.as_str()),
            &conduit_core::BootId::from(self.image_boot_id.as_str()),
            Some(&conduit_core::ActivePlayId::from(
                self.image_active_play_id.as_str(),
            )),
            16,
        );
        field(&record, "sign_id", sign.sign_id.as_str())
    }

    pub fn firmware_build_id(&self) -> Option<&str> {
        self.firmware_build_id.as_deref()
    }

    fn verify_plan_fields(
        &self,
        record: &serde_json::Value,
        active_play: bool,
    ) -> Result<(), String> {
        for (name, expected) in [
            ("plan_id", self.plan_id.as_str()),
            ("fragment_id", self.fragment_id.as_str()),
            ("host_id", self.host_id.as_str()),
            ("boot_id", self.image_boot_id.as_str()),
        ] {
            field(record, name, expected)?;
        }
        if active_play {
            field(record, "active_play_id", &self.image_active_play_id)?;
        }
        Ok(())
    }

    fn capture_build_fields(&mut self, record: &serde_json::Value) -> Result<(), String> {
        self.firmware_build_id = Some(string(record, "firmware_build_id")?.to_owned());
        self.source_document_id = Some(string(record, "source_document_id")?.to_owned());
        self.checked_form_id = Some(string(record, "checked_form_id")?.to_owned());
        self.expanded_form_id = Some(string(record, "expanded_form_id")?.to_owned());
        Ok(())
    }

    fn verify_build_fields(&self, record: &serde_json::Value) -> Result<(), String> {
        for (name, expected) in [
            ("firmware_build_id", self.firmware_build_id.as_deref()),
            ("source_document_id", self.source_document_id.as_deref()),
            ("checked_form_id", self.checked_form_id.as_deref()),
            ("expanded_form_id", self.expanded_form_id.as_deref()),
        ] {
            field(
                record,
                name,
                expected.ok_or_else(|| "Pico boot sign was not verified first".to_owned())?,
            )?;
        }
        Ok(())
    }
}

fn parse(line: &str, schema: &str) -> Result<serde_json::Value, String> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed Pico sign JSON: {error}; {line}"))?;
    field(&record, "schema", schema)?;
    Ok(record)
}

fn verify_runtime(record: &serde_json::Value, runtime: &PicoRuntimeIdentity) -> Result<(), String> {
    field(record, "runtime_boot_id", &runtime.boot_id)?;
    field(record, "runtime_active_play_id", &runtime.active_play_id)
}

fn field(record: &serde_json::Value, name: &str, expected: &str) -> Result<(), String> {
    let actual = string(record, name)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "Pico sign `{name}` mismatch: expected {expected}, got {actual}"
        ))
    }
}

fn string<'a>(record: &'a serde_json::Value, name: &str) -> Result<&'a str, String> {
    record[name]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Pico sign missing string `{name}`"))
}
