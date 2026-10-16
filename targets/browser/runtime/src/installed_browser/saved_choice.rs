//! Generic browser realization of a visibly saved finite choice.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserBack,
};
use conduit_core::{
    kind_id, resource_requirement, ConfigurationValue, HostCallContractId, HostCallRequirement,
    PlannedGear, StructuredInfoValue, StructuredInfoValueShape as Shape,
};
use conduit_face::{
    ActionAvailability, ApplicationEventKind, FaceMechanism, FieldKind, FormField, SelectOption,
    SemanticAction, SemanticApplicationView, SemanticFaceNode,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) const HOST_CALL: &str = "conduit.host/browser-saved-choice@1";
pub(crate) const IMPLEMENTATION: &str = "browser/kernel-saved-choice@1";
pub(crate) const RESOURCE_CLASS: &str = "conduit.resource/saved-choice-slot@1";
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct SavedChoiceOption {
    pub identity: String,
    pub label: String,
}
#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct SavedChoiceConfiguration {
    pub label: String,
    pub key: String,
    pub initial: String,
    pub options: Vec<SavedChoiceOption>,
    pub view: Vec<u8>,
}

impl SavedChoiceConfiguration {
    pub(crate) fn validate_selection(&self, bytes: &[u8]) -> Result<(), String> {
        let identity =
            core::str::from_utf8(bytes).map_err(|_| "saved choice output is not UTF-8")?;
        if self
            .options
            .iter()
            .any(|option| option.identity == identity)
        {
            Ok(())
        } else {
            Err("saved choice output is absent from authored options".into())
        }
    }
}

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::saved_choice_contract(),
        conduit_semantic_catalog::SAVED_CHOICE_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/saved-choice@1",
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(HOST_CALL),
            target_kind: Some(kind_id("input/saved-choice")),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_semantic_catalog::SAVED_CHOICE_MAXIMUM_ID_BYTES,
        }],
        vec![
            resource_requirement(conduit_core::FACE_RESOURCE_CLASS, 1),
            resource_requirement(RESOURCE_CLASS, 1),
        ],
        Vec::new(),
    )
}
fn checked_text(value: &str, maximum: u32) -> Result<String, String> {
    if value.is_empty() || value.len() > maximum as usize || value.contains('\0') {
        return Err("saved choice text is empty, contains NUL, or exceeds its bound".into());
    }
    Ok(value.into())
}
pub(crate) fn configuration(placement: &PlannedGear) -> Result<SavedChoiceConfiguration, String> {
    let field = |key: &str| {
        placement
            .configuration
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
            .ok_or_else(|| format!("missing saved choice {key}"))
    };
    let text = |key: &str, max: u32| match field(key)? {
        ConfigurationValue::Text(value) => checked_text(value, max),
        _ => Err(format!("saved choice {key} must be Text")),
    };
    let label = text("label", 128)?;
    let key = text("key", 128)?;
    let initial = text("initial", 64)?;
    let ConfigurationValue::Structured(options) = field("options")? else {
        return Err("saved choice options must be structured".into());
    };
    if options.profile()
        != conduit_semantic_catalog::saved_choice_options_type()
            .profile()
            .map_err(|e| format!("{e:?}"))?
            .value_kind()
    {
        return Err("saved choice options have the wrong exact type".into());
    }
    let value = StructuredInfoValue::from_canonical_bytes(options.canonical_value())
        .map_err(|e| format!("invalid saved choice options: {e:?}"))?;
    let Shape::Collection(values) = value.shape() else {
        return Err("saved choice options must be a finite sequence".into());
    };
    if values.is_empty() || values.len() > 16 {
        return Err("saved choice option count exceeds bound".into());
    }
    let mut options = Vec::<SavedChoiceOption>::with_capacity(values.len());
    for value in values {
        let Shape::Record(fields) = value.shape() else {
            return Err("saved choice option must be a record".into());
        };
        let get = |name: &str, max: u32| {
            let item = fields
                .iter()
                .find(|field| field.name() == name)
                .ok_or("missing saved choice option field")?;
            let Shape::Leaf(bytes) = item.value().shape() else {
                return Err("saved choice option field must be Text".to_owned());
            };
            checked_text(
                core::str::from_utf8(bytes).map_err(|_| "invalid choice UTF-8")?,
                max,
            )
        };
        let identity = get("identity", 64)?;
        let label = get("label", 128)?;
        if options.iter().any(|option| option.identity == identity) {
            return Err("saved choice option identities must be unique".into());
        }
        options.push(SavedChoiceOption { identity, label });
    }
    if !options.iter().any(|option| option.identity == initial) {
        return Err("saved choice initial identity is absent from options".into());
    }
    let view = SemanticApplicationView {
        revision: 0,
        root: SemanticFaceNode {
            key: "saved-choice".into(),
            mechanism: FaceMechanism::FormField(FormField {
                label: label.clone(),
                help: "Your choice is saved automatically in this browser.".into(),
                error: None,
                value: initial.clone(),
                value_capacity: 64,
                input_action: SemanticAction {
                    identity: "saved-choice.choose".into(),
                    label: "Choose".into(),
                    event: ApplicationEventKind::Change,
                    availability: ActionAvailability::Available,
                },
                kind: FieldKind::NamedSelect {
                    options: options
                        .iter()
                        .map(|option| SelectOption {
                            identity: option.identity.clone(),
                            label: option.label.clone(),
                        })
                        .collect(),
                },
            }),
            children: Vec::new(),
        },
    }
    .lower()
    .map_err(|e| format!("saved choice face: {e:?}"))?
    .encode()
    .map_err(|e| format!("saved choice encoding: {e:?}"))?;
    Ok(SavedChoiceConfiguration {
        label,
        key,
        initial,
        options,
        view,
    })
}
fn prepare(placement: &PlannedGear, values: &mut HostedValueStore) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    configuration(placement)?;
    let request = values
        .store(&[0])
        .map_err(|e| format!("saved choice request: {e:?}"))?;
    Ok(BrowserBack::installed_step(SavedChoiceBack {
        request,
        pending: false,
        next: 0,
    }))
}
struct SavedChoiceBack {
    request: ValueRef,
    pending: bool,
    next: u32,
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
impl<const P: usize> StepBack<P> for SavedChoiceBack {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(self.next) {
                return fail(51);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed {
                return fail(52);
            }
            let Some(output) = outcome.output else {
                return fail(53);
            };
            if output.value.byte_len == 0 || output.value.byte_len > 64 {
                return fail(54);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(next) = self.next.checked_add(1) else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::IdentityCapacityExhausted,
                    detail: 51,
                });
            };
            io.consume_host_completion()
                .expect("observed saved choice completion");
            io.send(PortId(0), output.value)
                .expect("ready saved choice output");
            self.pending = false;
            self.next = next;
        }
        if !self.pending {
            io.request_host_call(
                RequestId(self.next),
                HostCallId(0),
                BoundedValueRef::new(self.request, 1).expect("one byte saved choice request"),
            )
            .expect("saved choice Host Call");
            self.pending = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}
