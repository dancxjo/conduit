//! Current Host admission for domain-owned checked Todo spoken wording.
use super::parsec_source_plan::{expanded, Producer};
use conduit_core::{CapabilityOffer, ConfigurationValue, HostAdvertisement};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedSyntaxDocument, PortableExpressionProgram,
    StartupCatalog,
};
use conduit_todo_face::{
    PreparedTodoSpokenTemplate, TodoFaceFacts, TodoSpokenWording, TODO_SPOKEN_TEMPLATE_SOURCE,
};
fn checked() -> Result<CheckedSyntaxDocument, String> {
    check_syntax_document(
        &parse_syntax_document(TODO_SPOKEN_TEMPLATE_SOURCE),
        &StartupCatalog::new(),
    )
    .map_err(|e| format!("Todo spoken source: {e:?}"))
}
fn offer(source: &CheckedSyntaxDocument, name: &str) -> Result<CapabilityOffer, String> {
    let source = expanded(source, name)?;
    let [gear] = source.expanded.gears.as_slice() else {
        return Err("Todo wording must select one expression".into());
    };
    let ConfigurationValue::Text(program) = &gear.configuration[0].value else {
        return Err("Todo wording has no checked program".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(program)
        .map_err(|e| format!("Todo wording program: {e:?}"))?;
    conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
        .map_err(|e| format!("Todo wording offer: {e:?}"))
}
pub fn required_todo_spoken_template_offers() -> Result<Vec<CapabilityOffer>, String> {
    let source = checked()?;
    [
        "spoken/todo-brief-template",
        "spoken/todo-expanded-template",
    ]
    .into_iter()
    .map(|name| offer(&source, name))
    .collect()
}
pub struct TodoSpokenTemplateSourcePlan {
    checked: CheckedSyntaxDocument,
    hosts: [HostAdvertisement; 1],
    producer: Producer,
}
impl TodoSpokenTemplateSourcePlan {
    pub fn prepare(host: &HostAdvertisement, name: &str) -> Result<Self, String> {
        if !matches!(
            name,
            "spoken/todo-brief-template" | "spoken/todo-expanded-template"
        ) {
            return Err("unknown Todo spoken template".into());
        }
        let checked = checked()?;
        let offer = offer(&checked, name)?;
        if host
            .capabilities
            .iter()
            .find(|x| x.capability_id == offer.capability_id)
            != Some(&offer)
        {
            return Err("current Host lacks exact Todo wording offer".into());
        }
        let hosts = [host.clone()];
        let producer = Producer::prepare_bounded(&checked, &hosts, name, 131_072)?;
        Ok(Self {
            checked,
            hosts,
            producer,
        })
    }
    pub fn evaluator(&self) -> Result<PreparedTodoSpokenTemplate, String> {
        let custody = self.producer.custody(&self.checked, &self.hosts)?;
        PreparedTodoSpokenTemplate::prepare(&custody)
            .map_err(|e| format!("Todo wording custody: {e:?}"))
    }
}
impl crate::StdHost {
    pub fn prepare_todo_spoken_wording(
        &self,
        face: &conduit_face::Face,
        entry: &str,
        page: &[&str],
        locale: &str,
    ) -> Result<TodoSpokenWording, String> {
        let source = TodoSpokenTemplateSourcePlan::prepare(self.advertisement(), entry)?;
        let facts = TodoFaceFacts::prepare(face).map_err(|e| format!("Todo facts: {e:?}"))?;
        source
            .evaluator()?
            .evaluate(face, &facts, page, locale)
            .map_err(|e| format!("Todo wording: {e:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StdHost, StdHostConfig};
    #[test]
    fn current_host_requires_exact_offers_for_both_todo_styles() {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: "proof/todo-wording".into(),
            boot_id: "proof/todo-wording/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        assert!(TodoSpokenTemplateSourcePlan::prepare(
            host.advertisement(),
            "spoken/todo-brief-template"
        )
        .is_err());
        let mut advertisement = host.advertisement().clone();
        let offers = required_todo_spoken_template_offers().unwrap();
        advertisement.capabilities.extend(offers.clone());
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let host = StdHost::from_advertisement(advertisement).unwrap();
        for name in [
            "spoken/todo-brief-template",
            "spoken/todo-expanded-template",
        ] {
            TodoSpokenTemplateSourcePlan::prepare(host.advertisement(), name)
                .unwrap()
                .evaluator()
                .unwrap();
        }
        let mut stale = host.advertisement().clone();
        stale
            .capabilities
            .iter_mut()
            .find(|x| x.capability_id == offers[0].capability_id)
            .unwrap()
            .artifact_id = "forged/todo-wording".into();
        assert!(
            TodoSpokenTemplateSourcePlan::prepare(&stale, "spoken/todo-brief-template").is_err()
        );
    }
    #[test]
    #[ignore = "requires actual retained owner Face through CONDUIT_TODO_TEMPLATE_FACE"]
    fn actual_owner_face_uses_current_host_selected_todo_source() {
        let path = std::env::var("CONDUIT_TODO_TEMPLATE_FACE").unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let face: conduit_face::Face = serde_json::from_value(value["face"].clone()).unwrap();
        let base = StdHost::new_with_config(StdHostConfig {
            host_id: "proof/todo-current-host".into(),
            boot_id: "proof/todo-current-host/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        let mut advertisement = base.advertisement().clone();
        advertisement
            .capabilities
            .extend(required_todo_spoken_template_offers().unwrap());
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let host = StdHost::from_advertisement(advertisement).unwrap();
        for entry in [
            "spoken/todo-brief-template",
            "spoken/todo-expanded-template",
        ] {
            let words = host
                .prepare_todo_spoken_wording(&face, entry, &["todo/item/task-1"], "en")
                .unwrap();
            assert_eq!(words.source_face(), face.identity.as_str());
            assert_eq!(words.revision(), face.revision);
            assert_eq!(words.remaining(), 1);
            assert_eq!(words.completed(), 0);
            assert!(words.wording().contains("Crème brûlée — 茶 🍵?"));
        }
    }
}
