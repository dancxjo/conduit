//! One exact checked source producer over opaque Todo Face facts.
//! Wording preparation proves no speech, Show, effect or traversal-control grant.
use crate::{TodoFaceFacts, TodoFactRefusal};
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::{
    ConfigurationValue, PlanId, PlotIdentity, StructuredFieldValue, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_face::{Face, FaceActionAvailability, PreparedCheckedSourcePlan};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog, PURE_EXPRESSION_REVISION,
};
pub const TODO_SPOKEN_TEMPLATE_SOURCE: &str = include_str!("../spoken_templates.conduit");
#[derive(Debug)]
pub enum TodoTemplateRefusal {
    Source,
    Shape,
    Facts(TodoFactRefusal),
    Locale,
    Bound,
    Evaluation,
}
pub struct PreparedTodoSpokenTemplate {
    program: PortableExpressionProgram,
    identity: String,
    plot: PlotIdentity,
    plan: PlanId,
}
pub struct TodoSpokenWording {
    identity: String,
    source_face: String,
    revision: u64,
    remaining: u64,
    completed: u64,
    plan: PlanId,
    plot: PlotIdentity,
    wording: String,
    actions: Vec<String>,
}
impl TodoSpokenWording {
    pub fn version(&self) -> u16 {
        1
    }
    pub fn locale(&self) -> &str {
        "en"
    }
    pub fn source(&self) -> &str {
        TODO_SPOKEN_TEMPLATE_SOURCE
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn source_face(&self) -> &str {
        &self.source_face
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
    pub fn completed(&self) -> u64 {
        self.completed
    }
    pub fn plan(&self) -> &PlanId {
        &self.plan
    }
    pub fn plot(&self) -> &PlotIdentity {
        &self.plot
    }
    pub fn wording(&self) -> &str {
        &self.wording
    }
    pub fn actions(&self) -> &[String] {
        &self.actions
    }
}
impl PreparedTodoSpokenTemplate {
    pub fn prepare(selected: &PreparedCheckedSourcePlan<'_>) -> Result<Self, TodoTemplateRefusal> {
        use TodoTemplateRefusal as R;
        let name = selected.expanded().expanded.name.as_str();
        let identity = match name {
            "spoken/todo-brief-template" => "spoken/todo-brief@1",
            "spoken/todo-expanded-template" => "spoken/todo-expanded@1",
            _ => return Err(R::Source),
        };
        let checked = check_syntax_document(
            &parse_syntax_document(TODO_SPOKEN_TEMPLATE_SOURCE),
            &StartupCatalog::new(),
        )
        .map_err(|_| R::Source)?;
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
            .map_err(|_| R::Source)?;
        if selected.expanded() != &expanded {
            return Err(R::Source);
        }
        let [gear] = expanded.expanded.gears.as_slice() else {
            return Err(R::Shape);
        };
        if gear.kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION {
            return Err(R::Shape);
        }
        let [field] = gear.configuration.as_slice() else {
            return Err(R::Shape);
        };
        let ("program", ConfigurationValue::Text(encoded)) = (field.key.as_str(), &field.value)
        else {
            return Err(R::Shape);
        };
        let program =
            PortableExpressionProgram::from_canonical_hex(encoded).map_err(|_| R::Shape)?;
        Ok(Self {
            program,
            identity: identity.into(),
            plan: selected.plan().plan_id.clone(),
            plot: PlotIdentity {
                source_document_id: expanded.expanded.source_document_id.clone(),
                checked_plot_id: expanded.expanded.checked_plot_id.clone(),
                expanded_plot_id: expanded.expanded.expanded_plot_id.clone(),
            },
        })
    }
    pub fn evaluate(
        &self,
        face: &Face,
        facts: &TodoFaceFacts<'_>,
        page: &[&str],
        locale: &str,
    ) -> Result<TodoSpokenWording, TodoTemplateRefusal> {
        use TodoTemplateRefusal as R;
        if locale != "en" {
            return Err(R::Locale);
        }
        facts.validate_current(face).map_err(R::Facts)?;
        let items = facts.remaining_page(page).map_err(R::Facts)?;
        let names = items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        let mut controls = Vec::new();
        let mut ids = Vec::new();
        let mut warnings = Vec::new();
        for action in &face.actions {
            if action.target == facts.list().identity
                || items.iter().any(|item| item.identity == action.target)
            {
                // Unselected detail controls are not spoken in a remaining-items page.
                if !matches!(
                    action.disclosure,
                    conduit_face::FaceDisclosureLevel::CurrentAction
                        | conduit_face::FaceDisclosureLevel::Primary
                        | conduit_face::FaceDisclosureLevel::Context
                ) {
                    continue;
                }
                if let FaceActionAvailability::Unavailable { explanation, .. } =
                    &action.availability
                {
                    warnings.push(explanation.as_str());
                } else if action.intent == "todo/complete@1"
                    && items.iter().any(|item| item.identity == action.target)
                {
                    let action = facts.action(face, &action.identity).map_err(R::Facts)?;
                    controls.push(format!(
                        "{} {}",
                        action.name,
                        face.subjects
                            .iter()
                            .find(|s| s.identity == action.target)
                            .ok_or(R::Shape)?
                            .name
                    ));
                    ids.push(action.identity.clone());
                }
            }
        }
        // All admitted essential warnings survive either rhetorical style.
        for subject in face
            .subjects
            .iter()
            .filter(|s| s.role == conduit_face::FaceRole::Status)
        {
            for text in face.text.iter().filter(|t| t.subject == subject.identity) {
                if subject.identity != "todo/status" {
                    if !face.disclosures.iter().any(|d| {
                        d.subject == subject.identity
                            && matches!(
                                d.level,
                                conduit_face::FaceDisclosureLevel::Primary
                                    | conduit_face::FaceDisclosureLevel::Context
                            )
                    }) {
                        return Err(R::Facts(TodoFactRefusal::Disclosure));
                    }
                    warnings.push(&text.text);
                }
            }
        }
        let controls = controls.join("; ");
        let warnings = warnings.join(" ");
        if names.len() > 512 || controls.len() > 512 || warnings.len() > 2048 {
            return Err(R::Bound);
        }
        let strings = [
            ("list_name", facts.list().name.as_str()),
            ("locale", locale),
            ("items", names.as_str()),
            ("actions", controls.as_str()),
            ("warnings", warnings.as_str()),
        ];
        let StructuredInfoTypeShape::Record { fields, .. } = self.program.input_type.shape() else {
            return Err(R::Shape);
        };
        let mut values = Vec::new();
        for field in fields {
            let bytes = match field.name() {
                "remaining" => facts.remaining().to_le_bytes().to_vec(),
                "completed" => facts.completed().to_le_bytes().to_vec(),
                name => strings
                    .iter()
                    .find(|(key, _)| *key == name)
                    .ok_or(R::Shape)?
                    .1
                    .as_bytes()
                    .to_vec(),
            };
            values.push(
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(field.value_type().clone(), bytes)
                        .map_err(|_| R::Bound)?,
                )
                .map_err(|_| R::Shape)?,
            );
        }
        let input = StructuredInfoValue::record(self.program.input_type.clone(), values)
            .map_err(|_| R::Shape)?
            .canonical_bytes()
            .map_err(|_| R::Shape)?;
        let output = self.program.evaluate(&input).map_err(|_| R::Evaluation)?;
        let output =
            StructuredInfoValue::from_canonical_bytes(&output).map_err(|_| R::Evaluation)?;
        if output.value_type() != &self.program.output_type {
            return Err(R::Shape);
        }
        let bytes = |name: &str| -> Result<&[u8], TodoTemplateRefusal> {
            let StructuredInfoValueShape::Record(fields) = output.shape() else {
                return Err(R::Shape);
            };
            let field = fields
                .iter()
                .find(|field| field.name() == name)
                .ok_or(R::Evaluation)?
                .value();
            let StructuredInfoValueShape::Leaf(bytes) = field.shape() else {
                return Err(R::Shape);
            };
            Ok(bytes)
        };
        if bytes("version")? != 1_u16.to_le_bytes() {
            return Err(R::Evaluation);
        }
        if bytes("accepted")? != [1] {
            return Err(R::Bound);
        }
        if bytes("identity")? != self.identity.as_bytes()
            || bytes("remaining")? != facts.remaining().to_le_bytes()
            || bytes("completed")? != facts.completed().to_le_bytes()
        {
            return Err(R::Evaluation);
        }
        let text = |name: &str| -> Result<String, TodoTemplateRefusal> {
            core::str::from_utf8(bytes(name)?)
                .map(ToString::to_string)
                .map_err(|_| R::Evaluation)
        };
        let mut segments = Vec::new();
        let prefix = text("j")?;
        if !prefix.is_empty() {
            segments.push(prefix);
        }
        segments.push(format!("{}{}", text("a")?, text("k")?));
        segments.push(text("b")?);
        segments.push(text("c")?);
        for (label, value) in [("g", "d"), ("h", "e"), ("", "f")] {
            if !label.is_empty() {
                let label = text(label)?;
                if !label.is_empty() {
                    segments.push(label);
                }
            }
            let value = text(value)?;
            if !value.is_empty() {
                segments.push(value);
            }
        }
        let wording = segments.join(" ");
        if wording.len() > 8192 {
            return Err(R::Bound);
        }
        Ok(TodoSpokenWording {
            identity: self.identity.clone(),
            source_face: face.identity.as_str().into(),
            revision: face.revision,
            remaining: facts.remaining(),
            completed: facts.completed(),
            plan: self.plan.clone(),
            plot: self.plot.clone(),
            wording,
            actions: ids,
        })
    }
}
