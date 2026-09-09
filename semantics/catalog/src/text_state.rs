//! Reusable bounded text editing and line-submission semantics.

use alloc::vec::Vec;
use alloc::{format, string::ToString, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
    TEXT_PRESENTATION_VALUE_KIND,
};

pub const TEXT_EDIT_KIND: &str = "text/edit";
pub const TEXT_SUBMIT_LINES_KIND: &str = "text/submit-lines";
pub const TEXT_EDIT_REVISION: &str = "conduit.text/edit@1";
pub const TEXT_SUBMIT_LINES_REVISION: &str = "conduit.text/submit-lines@1";
pub const MAXIMUM_EDITED_TEXT_BYTES: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStateMode {
    Edit,
    Submit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStateRefusal {
    InvalidCapacity,
    InvalidUtf8,
    CapacityExhausted,
}

/// Pre-admitted retained text storage shared by every compatible Host.
pub struct BoundedTextState {
    mode: TextStateMode,
    text: Vec<u8>,
    pending_clear: bool,
}

impl BoundedTextState {
    pub fn new(mode: TextStateMode, maximum_bytes: usize) -> Result<Self, TextStateRefusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_EDITED_TEXT_BYTES as usize {
            return Err(TextStateRefusal::InvalidCapacity);
        }
        Ok(Self {
            mode,
            text: Vec::with_capacity(maximum_bytes),
            pending_clear: false,
        })
    }

    pub fn apply(&mut self, fragment: &[u8]) -> Result<Option<&[u8]>, TextStateRefusal> {
        if self.pending_clear {
            self.text.clear();
            self.pending_clear = false;
        }
        let fragment = core::str::from_utf8(fragment).map_err(|_| TextStateRefusal::InvalidUtf8)?;
        if fragment == "\n" {
            return match self.mode {
                TextStateMode::Edit => Ok(Some(&self.text)),
                TextStateMode::Submit if self.text.is_empty() => Ok(None),
                TextStateMode::Submit => {
                    self.pending_clear = true;
                    Ok(Some(&self.text))
                }
            };
        }
        if fragment == "\u{8}" {
            if let Some((index, _)) = core::str::from_utf8(&self.text)
                .ok()
                .and_then(|text| text.char_indices().next_back())
            {
                self.text.truncate(index);
            }
            return Ok(matches!(self.mode, TextStateMode::Edit).then_some(&self.text));
        }
        if self
            .text
            .len()
            .checked_add(fragment.len())
            .is_none_or(|length| length > self.text.capacity())
        {
            return Err(TextStateRefusal::CapacityExhausted);
        }
        self.text.extend_from_slice(fragment.as_bytes());
        Ok(matches!(self.mode, TextStateMode::Edit).then_some(&self.text))
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn submission_reuses_fixed_storage_and_clears_between_lines() {
        let mut state = BoundedTextState::new(TextStateMode::Submit, 5).unwrap();
        assert_eq!(state.apply(b"one").unwrap(), None);
        assert_eq!(state.apply(b"\n").unwrap(), Some(b"one".as_slice()));
        assert_eq!(state.apply(b"two").unwrap(), None);
        assert_eq!(state.apply(b"\n").unwrap(), Some(b"two".as_slice()));
        assert_eq!(
            state.apply(b"123456"),
            Err(TextStateRefusal::CapacityExhausted)
        );
    }

    #[test]
    fn editing_backspace_is_utf8_safe_and_capacity_is_exact() {
        let mut state = BoundedTextState::new(TextStateMode::Edit, 4).unwrap();
        assert_eq!(state.apply("éa".as_bytes()).unwrap(), Some("éa".as_bytes()));
        assert_eq!(state.apply(b"\x08").unwrap(), Some("é".as_bytes()));
        assert_eq!(state.apply(b"bc").unwrap(), Some("ébc".as_bytes()));
        assert_eq!(state.apply(b"d"), Err(TextStateRefusal::CapacityExhausted));
    }
}

fn contract(kind: &str, revision: &str, output: &str, summary: &str) -> StandardKindContract {
    let _ = revision;
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: if kind == TEXT_EDIT_KIND {
            "Text editing"
        } else {
            "Line submission"
        }
        .to_string(),
        summary: summary.to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("fragment"),
            value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: false },
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id(output),
            value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: false },
        }],
        configuration: vec![StandardConfigurationField {
            key: "maximum-bytes".to_string(),
            default_value: ConfigurationValue::U64(256),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAXIMUM_EDITED_TEXT_BYTES as u64,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: MAXIMUM_EDITED_TEXT_BYTES,
        },
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: format!("state: {kind}(maximum-bytes = 256)"),
    }
}

pub fn text_edit_contract() -> StandardKindContract {
    contract(
        TEXT_EDIT_KIND,
        TEXT_EDIT_REVISION,
        "current",
        "Retain and emit bounded current text after each edit fragment.",
    )
}

pub fn text_submit_lines_contract() -> StandardKindContract {
    contract(
        TEXT_SUBMIT_LINES_KIND,
        TEXT_SUBMIT_LINES_REVISION,
        "submitted",
        "Accumulate bounded text and emit one message for each Enter fragment.",
    )
}

#[cfg(feature = "form-catalog")]
pub fn install_text_state_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    for (contract, revision) in [
        (text_edit_contract(), TEXT_EDIT_REVISION),
        (text_submit_lines_contract(), TEXT_SUBMIT_LINES_REVISION),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: vec![StartupParameterSignature {
                name: "maximum-bytes".to_string(),
                value_type: "Count".to_string(),
                default: Some("256".to_string()),
            }],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![ConfigurationField {
                    key: "maximum-bytes".to_string(),
                    default_value: ConfigurationValue::U64(256),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum: MAXIMUM_EDITED_TEXT_BYTES as u64,
                    },
                }],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
