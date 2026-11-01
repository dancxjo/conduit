use super::*;
use conduit_face::*;

struct Projection {
    subjects: Vec<FaceSubject>,
    relationships: Vec<FaceRelationship>,
    properties: Vec<FaceProperty>,
    text: Vec<FaceText>,
    actions: Vec<FaceAction>,
}
impl Projection {
    fn text(&mut self, text: String) {
        self.text.push(FaceText {
            subject: "host-config".into(),
            text,
        });
    }
    fn collection(&mut self, count: usize) {
        self.subjects.push(FaceSubject {
            identity: "host-config/choices".into(),
            role: FaceRole::Collection,
            name: "Package choices on this page".into(),
        });
        self.relationships.push(FaceRelationship {
            source: "host-config".into(),
            target: "host-config/choices".into(),
            kind: FaceRelationshipKind::Contains,
        });
        self.properties.push(FaceProperty {
            subject: "host-config/choices".into(),
            name: "item-count".into(),
            value: FacePropertyValue::Count(count as u64),
        });
    }
    fn choice(&mut self, id: &str, label: &str, order: usize) {
        self.subjects.push(FaceSubject {
            identity: id.into(),
            role: FaceRole::Item,
            name: label.into(),
        });
        self.relationships.push(FaceRelationship {
            source: "host-config/choices".into(),
            target: id.into(),
            kind: FaceRelationshipKind::Contains,
        });
        self.properties.push(FaceProperty {
            subject: id.into(),
            name: "order".into(),
            value: FacePropertyValue::Count(order as u64),
        });
        self.action(id, label);
        self.actions.last_mut().unwrap().target = id.into();
    }
    fn action(&mut self, id: &str, label: &str) {
        self.actions.push(FaceAction {
            identity: id.into(),
            intent: format!("conduit.intent/{id}@1"),
            target: "host-config".into(),
            name: label.into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        });
    }
    fn field(&mut self, id: &str, label: &str, current: &str, maximum: u32) -> Result<(), String> {
        self.subjects.push(FaceSubject {
            identity: id.into(),
            role: FaceRole::TextEntry,
            name: label.into(),
        });
        self.relationships.push(FaceRelationship {
            source: "host-config".into(),
            target: id.into(),
            kind: FaceRelationshipKind::Contains,
        });
        let argument = FaceActionArgument::text("value".into(), label.into(), 0, maximum)
            .map_err(|_| "Invalid Host field contract")?;
        if !current.is_empty() {
            self.properties.push(FaceProperty {
                subject: id.into(),
                name: "current-value".into(),
                value: FacePropertyValue::TypedValue {
                    contract: argument.contract.clone(),
                    bytes: current.as_bytes().to_vec(),
                },
            });
        }
        self.properties.push(FaceProperty {
            subject: id.into(),
            name: "value-change-action".into(),
            value: FacePropertyValue::Identity(id.into()),
        });
        self.actions.push(FaceAction {
            identity: id.into(),
            intent: format!("conduit.intent/{id}@1"),
            target: id.into(),
            name: label.into(),
            arguments: vec![argument],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        });
        Ok(())
    }
    fn navigation(&mut self, page: usize, count: usize) {
        if page > 0 {
            self.action("host-config/previous", "Previous page");
        }
        if (page + 1) * PAGE_SIZE < count {
            self.action("host-config/next", "Next page");
        }
    }
}
pub(super) fn project(
    e: &HostConfigurationEncounter<'_>,
    b: &HostConfigurationEncounterBasis,
) -> Result<Face, String> {
    let x = &b.execution;
    if b.encounter_id.is_empty()
        || b.encounter_id.len() > 128
        || b.host_id.as_str().is_empty()
        || b.boot_id.as_str().is_empty()
        || x.body_id.is_none()
        || x.wake_id.is_none()
        || x.source_document_id.is_none()
        || x.checked_plot_id.is_none()
        || x.expanded_plot_id.is_none()
        || x.plan_id.is_none()
        || x.active_play_id.is_none()
    {
        return Err("Host authoring requires actual complete execution basis".into());
    }
    let mut p = Projection {
        subjects: vec![FaceSubject {
            identity: "host-config".into(),
            role: FaceRole::Plot,
            name: "Host configuration".into(),
        }],
        relationships: vec![],
        properties: vec![],
        text: vec![],
        actions: vec![],
    };
    for (name, value) in [
        ("host-id", b.host_id.as_str()),
        ("boot-id", b.boot_id.as_str()),
        ("encounter-id", b.encounter_id.as_str()),
    ] {
        p.properties.push(FaceProperty {
            subject: "host-config".into(),
            name: name.into(),
            value: FacePropertyValue::Identity(value.into()),
        });
    }
    p.text("Checked construction intent only. Configuration does not initialize equipment, grant authority or establish Ready.".into());
    match e.page {
        Page::Targets(page) => {
            let targets = e.packages.target_descriptors();
            p.collection(
                targets
                    .len()
                    .saturating_sub(page * PAGE_SIZE)
                    .min(PAGE_SIZE),
            );
            for (i, t) in targets
                .iter()
                .enumerate()
                .skip(page * PAGE_SIZE)
                .take(PAGE_SIZE)
            {
                p.choice(&format!("host-config/target/{i}"), &t.label, i);
                p.text(format!("Target: {}", t.key()));
            }
            p.navigation(page, targets.len());
        }
        Page::Bases(page) => {
            let offers = draft::offers(e)?;
            p.collection(offers.len().saturating_sub(page * PAGE_SIZE).min(PAGE_SIZE));
            for (i, o) in offers
                .iter()
                .enumerate()
                .skip(page * PAGE_SIZE)
                .take(PAGE_SIZE)
            {
                p.choice(
                    &format!("host-config/add/{i}"),
                    &format!("Select {}", o.offer.base_kind),
                    i,
                );
                p.text(format!("Implementation: {}", o.offer.implementation_id));
            }
            p.navigation(page, offers.len());
        }
        Page::Initialization(i) => {
            let c = e.configuration.as_ref().ok_or("No Host draft")?;
            let base = c.bases.get(i).ok_or("Unknown selected Base")?;
            let contract = e
                .catalog
                .initialization_contracts
                .get(
                    base.implementation
                        .as_ref()
                        .ok_or("No exact implementation")?,
                )
                .ok_or("No initialization contract")?;
            if contract.fields.len() > 16 {
                return Err("Initialization contract exceeds encounter field bound".into());
            }
            p.text(format!(
                "Explicit initialization: {}. All fields must be entered; defaults are hints only.",
                base.kind
            ));
            for field in &contract.fields {
                let current = base
                    .initialization
                    .as_ref()
                    .and_then(|v| v.values.get(&field.key))
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_default();
                p.field(
                    &format!("host-config/field/{}", field.key),
                    &field.key,
                    &current,
                    1024,
                )?;
                p.text(draft::bounded_text(
                    &format!("{} rule: {:?}", field.key, field.rule),
                    768,
                ));
            }
            p.action(
                "host-config/clear-initialization",
                "Clear explicit initialization",
            );
        }
        Page::Source(page) => {
            let source = e.canonical_source()?;
            let chunks = chunks(&source);
            for chunk in chunks.iter().skip(page * PAGE_SIZE).take(PAGE_SIZE) {
                p.text(chunk.clone());
            }
            p.navigation(page, chunks.len());
        }
        Page::Main => {
            if let Some(c) = &e.configuration {
                p.field("host-config/name", "Host name", &c.name, 64)?;
                let key = draft::target_key(c, e)?;
                let t = e
                    .packages
                    .target_descriptor(&key)
                    .ok_or("Unknown Host target")?;
                p.text(format!("Target: {key}"));
                if !t.make_descriptors.is_empty() {
                    p.field(
                        "host-config/descriptor",
                        "Exact Make descriptor",
                        c.target.make_descriptor.as_deref().unwrap_or(""),
                        256,
                    )?;
                    p.text(draft::bounded_text(
                        &format!("Select one descriptor: {:?}", t.make_descriptors),
                        768,
                    ));
                }
                for key in draft::LIMITS {
                    p.field(
                        &format!("host-config/limit/{key}"),
                        key,
                        &draft::limit(&c.limits, key).to_string(),
                        20,
                    )?;
                    p.text(format!(
                        "{key}: target maximum {} (hint only)",
                        draft::limit(&t.maxima, key)
                    ));
                }
                for (i, base) in c.bases.iter().enumerate() {
                    p.text(format!(
                        "Selected {}: {}",
                        base.kind,
                        base.implementation.as_deref().unwrap_or("unresolved")
                    ));
                    p.action(
                        &format!("host-config/remove/{i}"),
                        &format!("Remove {}", base.kind),
                    );
                    if base.implementation.as_ref().is_some_and(|implementation| {
                        e.catalog
                            .initialization_contracts
                            .contains_key(implementation)
                    }) {
                        p.action(
                            &format!("host-config/initialize/{i}"),
                            &format!("Initialize {}", base.kind),
                        );
                    }
                }
                p.action("host-config/bases", "Choose Base implementation");
                p.action("host-config/check", "Check Host configuration");
                match e.checked() {
                    Ok(_) => p.action("host-config/source", "Inspect checked canonical Source"),
                    Err(error) => p.text(format!("Not checked: {error}")),
                }
            }
            p.action("host-config/targets", "Select target (replaces this draft)");
        }
    }
    if !matches!(e.page, Page::Main) {
        p.action("host-config/home", "Back to Host draft");
    }
    Face::new_with_semantics(
        e.revision,
        b.execution.clone(),
        p.subjects,
        p.relationships,
        p.properties,
        p.text,
        p.actions,
        vec![FaceDisclosure {
            subject: "host-config".into(),
            level: FaceDisclosureLevel::Primary,
        }],
    )
    .map_err(|err| format!("Host authoring Face: {err:?}"))
}
fn chunks(source: &str) -> Vec<String> {
    let mut result = vec![];
    let mut chunk = String::new();
    for ch in source.chars() {
        if chunk.len() + ch.len_utf8() > 768 {
            result.push(std::mem::take(&mut chunk));
        }
        chunk.push(ch);
    }
    if !chunk.is_empty() {
        result.push(chunk);
    }
    result
}
