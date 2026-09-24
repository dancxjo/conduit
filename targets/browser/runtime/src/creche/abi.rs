use super::session;
use crate::source_interaction::SourceInteractionEvidence;
use std::cell::RefCell;

pub(super) mod spore_abi;

#[derive(serde::Serialize)]
struct CrecheRefusal {
    schema: &'static str,
    disposition: &'static str,
    message: String,
}

// Admit the canonical Body advertisement plus bounded invitation/signature framing.
pub(super) const INPUT_BYTES: usize =
    conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize + 8 * 1_024;
pub(super) const OUTPUT_BYTES: usize = 32 * 1_024;
const STATUS_READY: i32 = 0;
pub(super) const ERROR_INPUT: i32 = -451;
const ERROR_BIRTH: i32 = -452;
pub(super) const ERROR_OUTPUT: i32 = -453;
const ERROR_INTERACTION: i32 = -454;
const STATUS_ABSENT: i32 = 1;
pub(super) const ERROR_SPORE: i32 = -455;
const ERROR_ADMISSION: i32 = -456;
pub(super) const ERROR_GRADUATION: i32 = -457;
const ERROR_RESTORE: i32 = -458;
const ERROR_INVENTORY: i32 = -459;
const ERROR_REVIEW: i32 = -460;

thread_local! {
    // WASM callers may capture memory.buffer before asking for the input pointer.
    // Preallocate there so pointer access cannot grow memory and detach that view.
    #[cfg(target_arch = "wasm32")]
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
    // Native static TLS consumes thread-stack headroom; allocate the same fixed
    // capacity once on ABI entry, before admission, without a large stack array.
    #[cfg(not(target_arch = "wasm32"))]
    static INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; INPUT_BYTES].into_boxed_slice());
    static OUTPUT: RefCell<[u8; OUTPUT_BYTES]> = const { RefCell::new([0; OUTPUT_BYTES]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
    static SOURCE_INTERACTION: RefCell<Option<SourceInteractionEvidence>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn conduit_creche_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_creche_input_capacity() -> usize {
    INPUT_BYTES
}

#[no_mangle]
pub extern "C" fn conduit_creche_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_creche_output_len() -> usize {
    OUTPUT_LEN.with(|length| *length.borrow())
}

#[no_mangle]
pub extern "C" fn conduit_creche_reviewed_inventory(source_length: usize) -> i32 {
    clear_output();
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = core::str::from_utf8(&input[..source_length])
            .map_err(|_| "reviewed form inventory is not UTF-8".to_string())
            .and_then(super::initial_forms::reviewed_inventory);
        let diagnostics = result.as_ref().err().map(|_| {
            core::str::from_utf8(&input[..source_length])
                .ok()
                .map(conduit_form::parse_syntax_document)
                .map(|syntax| {
                    syntax
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| {
                            serde_json::json!({
                                "code": diagnostic.code,
                                "message": diagnostic.message,
                                "span": {
                                    "start": diagnostic.span.start,
                                    "end": diagnostic.span.end,
                                    "line": diagnostic.span.line,
                                    "column": diagnostic.span.column,
                                    "endLine": diagnostic.span.end_line,
                                    "endColumn": diagnostic.span.end_column,
                                },
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        input[..source_length].fill(0);
        match result {
            Ok(inventory) => write_output(&inventory)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => {
                let diagnostics = diagnostics.unwrap_or_default();
                let code = if diagnostics.is_empty() {
                    "FormCheckRefused"
                } else {
                    "FormSyntaxInvalid"
                };
                if write_output(&serde_json::json!({
                    "schema": "conduit.creche/form-check-refusal@1",
                    "disposition": "refused",
                    "code": code,
                    "message": message,
                    "diagnostics": diagnostics,
                }))
                .is_err()
                {
                    ERROR_OUTPUT
                } else {
                    ERROR_INVENTORY
                }
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_review_initial_workload(
    host_length: usize,
    boot_length: usize,
    initial_forms_length: usize,
    source_length: usize,
) -> i32 {
    clear_output();
    let Some(identity_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    let Some(forms_end) = identity_length.checked_add(initial_forms_length) else {
        return ERROR_INPUT;
    };
    let Some(total_length) = forms_end.checked_add(source_length) else {
        return ERROR_INPUT;
    };
    if host_length == 0
        || boot_length == 0
        || initial_forms_length == 0
        || source_length == 0
        || total_length > INPUT_BYTES
    {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length])
                .map_err(|_| "Host identity is not UTF-8".to_string())?;
            let boot = core::str::from_utf8(&input[host_length..identity_length])
                .map_err(|_| "Boot identity is not UTF-8".to_string())?;
            let selection = core::str::from_utf8(&input[identity_length..forms_end])
                .map_err(|_| "initial Form selection is not UTF-8".to_string())?;
            let source = core::str::from_utf8(&input[forms_end..total_length])
                .map_err(|_| "reviewed form inventory is not UTF-8".to_string())?;
            let hosts = [super::initial_forms::reviewed_browser_host(
                source,
                conduit_core::HostId::from(host),
                conduit_core::BootId::from(boot),
            )?];
            super::review::review(
                source,
                selection,
                &hosts,
                &crate::installed_browser::local_bases(),
            )
        })();
        input[..total_length].fill(0);
        match result {
            Ok(review) => {
                let receipt = serde_json::json!({
                    "schema": "conduit.creche/form-workload-review@1",
                    "review": &review,
                    "requirements": {
                        "kinds": &review.required_kinds,
                        "resources": &review.required_resources,
                        "capabilities": &review.required_capabilities,
                    },
                });
                write_output(&receipt)
                    .map(|()| STATUS_READY)
                    .unwrap_or(ERROR_OUTPUT)
            }
            Err(message) => refuse(message, ERROR_REVIEW),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_admit_source_interaction(
    source_length: usize,
    sequence: u64,
) -> i32 {
    clear_output();
    SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = None);
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = crate::source_interaction::admit_source(&input[..source_length], sequence);
        input[..source_length].fill(0);
        match result {
            Ok(evidence) => {
                if write_output(&evidence).is_err() {
                    return ERROR_OUTPUT;
                }
                SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = Some(evidence));
                STATUS_READY
            }
            Err(message) => refuse(message, ERROR_INTERACTION),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_birth(
    host_length: usize,
    boot_length: usize,
    friendly_name_length: usize,
    initial_forms_length: usize,
    source_length: usize,
    birth_sequence: u64,
) -> i32 {
    clear_output();
    let interaction = SOURCE_INTERACTION.with(|slot| slot.borrow_mut().take());
    let Some(identity_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    let Some(metadata_length) = identity_length
        .checked_add(friendly_name_length)
        .and_then(|length| length.checked_add(initial_forms_length))
    else {
        return ERROR_INPUT;
    };
    let Some(total_length) = metadata_length.checked_add(source_length) else {
        return ERROR_INPUT;
    };
    if host_length == 0
        || boot_length == 0
        || friendly_name_length == 0
        || initial_forms_length == 0
        || source_length == 0
        || total_length > INPUT_BYTES
    {
        return ERROR_INPUT;
    }
    let Some(interaction) = interaction else {
        return refuse(
            "source interaction was not admitted before BIRTH".into(),
            ERROR_INTERACTION,
        );
    };
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length])
                .map_err(|_| "Host identity is not UTF-8".to_string())?;
            let boot = core::str::from_utf8(&input[host_length..identity_length])
                .map_err(|_| "Boot identity is not UTF-8".to_string())?;
            let friendly_name_end = identity_length + friendly_name_length;
            let forms_end = friendly_name_end + initial_forms_length;
            let friendly_name = core::str::from_utf8(&input[identity_length..friendly_name_end])
                .map_err(|_| "friendly name is not UTF-8".to_string())?;
            let initial_forms = core::str::from_utf8(&input[friendly_name_end..forms_end])
                .map_err(|_| "initial Form selection is not UTF-8".to_string())?;
            let source = core::str::from_utf8(&input[forms_end..total_length])
                .map_err(|_| "Body Form source is not UTF-8".to_string())?;
            session::birth(
                host,
                boot,
                friendly_name,
                initial_forms,
                source,
                birth_sequence,
                interaction,
            )
        })();
        input[..total_length].fill(0);
        match result {
            Ok(receipt) => write_output(&receipt)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => refuse(message, ERROR_BIRTH),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_attach_here(
    host_length: usize,
    boot_length: usize,
    sequence: u64,
) -> i32 {
    clear_output();
    let Some(total_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    if host_length == 0 || boot_length == 0 || total_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length])
                .map_err(|_| "Host identity is not UTF-8".to_string())?;
            let boot = core::str::from_utf8(&input[host_length..total_length])
                .map_err(|_| "Boot identity is not UTF-8".to_string())?;
            session::attach_here(host, boot, sequence)
        })();
        input[..total_length].fill(0);
        match result {
            Ok(receipt) => write_output(&receipt)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => refuse(message, ERROR_ADMISSION),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_leave_here(
    host_length: usize,
    boot_length: usize,
    sequence: u64,
) -> i32 {
    host_boot_action(host_length, boot_length, |host, boot| {
        session::leave_here(host, boot, sequence)
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_revoke_here(
    host_length: usize,
    boot_length: usize,
    sequence: u64,
) -> i32 {
    host_boot_action(host_length, boot_length, |host, boot| {
        session::revoke_here(host, boot, sequence)
    })
}

fn host_boot_action(
    host_length: usize,
    boot_length: usize,
    action: impl FnOnce(&str, &str) -> Result<super::protocol::BirthReceipt, String>,
) -> i32 {
    clear_output();
    let Some(total_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    if host_length == 0 || boot_length == 0 || total_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length])
                .map_err(|_| "Host identity is not UTF-8".to_string())?;
            let boot = core::str::from_utf8(&input[host_length..total_length])
                .map_err(|_| "Boot identity is not UTF-8".to_string())?;
            action(host, boot)
        })();
        input[..total_length].fill(0);
        match result {
            Ok(receipt) => write_output(&receipt)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => refuse(message, ERROR_ADMISSION),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_forget_local() -> i32 {
    clear_output();
    session::forget_local();
    STATUS_READY
}

#[no_mangle]
pub extern "C" fn conduit_creche_current() -> i32 {
    clear_output();
    match session::current() {
        Some(receipt) => write_output(&receipt)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        None => STATUS_ABSENT,
    }
}

#[no_mangle]
pub extern "C" fn conduit_creche_biography() -> i32 {
    clear_output();
    match session::biography() {
        Some(evidence) => write_output(&evidence)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        None => STATUS_ABSENT,
    }
}

#[no_mangle]
pub extern "C" fn conduit_creche_durable_snapshot() -> i32 {
    clear_output();
    match session::durable_snapshot() {
        Some(snapshot) => write_output(&snapshot)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        None => STATUS_ABSENT,
    }
}

#[no_mangle]
pub extern "C" fn conduit_creche_restore_durable(snapshot_length: usize) -> i32 {
    clear_output();
    if snapshot_length == 0 || snapshot_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = serde_json::from_slice(&input[..snapshot_length])
            .map_err(|_| "durable Crèche session is malformed".to_string())
            .and_then(session::restore_durable);
        input[..snapshot_length].fill(0);
        match result {
            Ok(receipt) => write_output(&receipt)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => refuse(message, ERROR_RESTORE),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_graduation_readiness() -> i32 {
    clear_output();
    match super::graduation::readiness() {
        Ok(receipt) => write_output(&receipt)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(message) => refuse(message, ERROR_GRADUATION),
    }
}

#[no_mangle]
pub extern "C" fn conduit_creche_graduate(choice: u32, sequence: u64) -> i32 {
    clear_output();
    match super::graduation::graduate(choice, sequence) {
        Ok(receipt) => write_output(&receipt)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(message) => refuse(message, ERROR_GRADUATION),
    }
}

pub(super) fn refuse(message: String, code: i32) -> i32 {
    if write_output(&CrecheRefusal {
        schema: "conduit.creche/refusal@1",
        disposition: "refused-before-lifecycle-change",
        message,
    })
    .is_err()
    {
        ERROR_OUTPUT
    } else {
        code
    }
}

pub(super) fn write_output(value: &impl serde::Serialize) -> Result<(), ()> {
    let encoded = serde_json::to_vec(value).map_err(|_| ())?;
    write_output_bytes(&encoded)
}

pub(super) fn write_output_bytes(encoded: &[u8]) -> Result<(), ()> {
    if encoded.len() > OUTPUT_BYTES {
        return Err(());
    }
    OUTPUT.with(|output| output.borrow_mut()[..encoded.len()].copy_from_slice(encoded));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    Ok(())
}

pub(super) fn clear_output() {
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
}

pub(super) fn take_input(length: usize) -> Result<Vec<u8>, i32> {
    if length == 0 || length > INPUT_BYTES {
        return Err(ERROR_INPUT);
    }
    Ok(INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let bytes = input[..length].to_vec();
        input[..length].fill(0);
        bytes
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_source_refusal_projects_canonical_parser_span() {
        let source = b"clock {";
        INPUT.with(|input| input.borrow_mut()[..source.len()].copy_from_slice(source));
        assert_eq!(
            conduit_creche_reviewed_inventory(source.len()),
            ERROR_INVENTORY
        );
        let bytes = OUTPUT.with(|output| {
            let length = OUTPUT_LEN.with(|length| *length.borrow());
            output.borrow()[..length].to_vec()
        });
        let refusal: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(refusal["schema"], "conduit.creche/form-check-refusal@1");
        assert_eq!(refusal["code"], "FormSyntaxInvalid");
        let span = &refusal["diagnostics"][0]["span"];
        assert!(span["start"].as_u64().is_some());
        assert_eq!(span["line"], 1);
        assert_eq!(span["column"], 1);
    }

    #[test]
    fn workload_review_exposes_requirements_without_mutating_the_durable_birth_receipt() {
        let source = "form clock {\n    tick: time/every(1s)\n}";
        let form = super::super::initial_forms::reviewed_inventory(source)
            .unwrap()
            .forms
            .remove(0);
        let selection = serde_json::to_vec(&[super::super::initial_forms::InitialFormSelection {
            name: form.name,
            source_document_id: form.source_document_id,
            checked_form_id: form.checked_form_id,
        }])
        .unwrap();
        let host = b"host/review";
        let boot = b"boot/review";
        let bytes = [
            host.as_slice(),
            boot.as_slice(),
            selection.as_slice(),
            source.as_bytes(),
        ]
        .concat();
        INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
        assert_eq!(
            conduit_creche_review_initial_workload(
                host.len(),
                boot.len(),
                selection.len(),
                source.len(),
            ),
            STATUS_READY
        );
        let response = OUTPUT.with(|output| {
            let length = OUTPUT_LEN.with(|length| *length.borrow());
            serde_json::from_slice::<serde_json::Value>(&output.borrow()[..length]).unwrap()
        });
        assert_eq!(response["schema"], "conduit.creche/form-workload-review@1");
        assert_eq!(
            response["requirements"]["kinds"][0],
            conduit_time::TIME_EVERY_KIND
        );
        assert!(response["requirements"]["resources"].is_array());
        assert!(response["requirements"]["capabilities"].is_array());
        assert_eq!(response["review"]["body_plan_created"], false);
        assert_eq!(response["review"]["authority_acquired"], false);
        assert!(response["review"].get("required_resources").is_none());
        assert!(response["review"].get("required_capabilities").is_none());
    }
}
