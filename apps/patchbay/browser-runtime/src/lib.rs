use conduit_core::{BootId, HostId, ImplementationId, PlanId};
use patchbay_model::{
    PatchbayBodyApplicationEntrance, PatchbayBodyAttachment, PatchbayBodyEntranceError,
    MAX_PATCHBAY_BODY_EVIDENCE_BYTES,
};
use std::cell::RefCell;

const INPUT_CAPACITY: usize = MAX_PATCHBAY_BODY_EVIDENCE_BYTES + 2_048;
const OUTPUT_CAPACITY: usize = 32 * 1_024;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -701;
const ERROR_EVIDENCE: i32 = -702;
const ERROR_HOSTED_MEMBERSHIP: i32 = -703;
const ERROR_OUTPUT: i32 = -704;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = RefCell::new(vec![0; INPUT_CAPACITY]);
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(OUTPUT_CAPACITY));
}

#[derive(serde::Serialize)]
struct EntranceProjection {
    schema: &'static str,
    relationship: &'static str,
    body_id: String,
    friendly_name: String,
    membership_revision: u64,
    current_host_id: Option<String>,
    current_boot_id: Option<String>,
    entries: Vec<EntranceEntry>,
}

#[derive(serde::Serialize)]
struct EntranceEntry {
    sequence: u64,
    heading: &'static str,
    explanation: String,
    evidence_sign_id: String,
}

#[no_mangle]
pub extern "C" fn conduit_patchbay_entrance_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_patchbay_entrance_input_capacity() -> usize {
    INPUT_CAPACITY
}

#[no_mangle]
pub extern "C" fn conduit_patchbay_entrance_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_patchbay_entrance_output_len() -> usize {
    OUTPUT.with(|output| output.borrow().len())
}

/// Opens one bounded durable Body-evidence document through the authoritative
/// Patchbay model. Hosted mode additionally proves that the exact browser
/// Host/Boot is current in that document; external mode never changes membership.
#[no_mangle]
pub extern "C" fn conduit_patchbay_open_body(
    mode: u32,
    host_length: usize,
    boot_length: usize,
    plan_length: usize,
    implementation_length: usize,
    evidence_length: usize,
) -> i32 {
    clear_output();
    let Some(metadata_length) = host_length
        .checked_add(boot_length)
        .and_then(|value| value.checked_add(plan_length))
        .and_then(|value| value.checked_add(implementation_length))
    else {
        return ERROR_INPUT;
    };
    let Some(total_length) = metadata_length.checked_add(evidence_length) else {
        return ERROR_INPUT;
    };
    if evidence_length == 0
        || evidence_length > MAX_PATCHBAY_BODY_EVIDENCE_BYTES
        || total_length > INPUT_CAPACITY
    {
        return ERROR_INPUT;
    }
    INPUT.with(|slot| {
        let mut input = slot.borrow_mut();
        let result = open_input(
            mode,
            &input[..host_length],
            &input[host_length..host_length + boot_length],
            &input[host_length + boot_length..host_length + boot_length + plan_length],
            &input[host_length + boot_length + plan_length..metadata_length],
            &input[metadata_length..total_length],
        );
        input[..total_length].fill(0);
        match result {
            Ok(projection) => write_output(&projection),
            Err(code) => code,
        }
    })
}

fn open_input(
    mode: u32,
    host: &[u8],
    boot: &[u8],
    plan: &[u8],
    implementation: &[u8],
    evidence: &[u8],
) -> Result<EntranceProjection, i32> {
    let entrance = match mode {
        1 if host.is_empty() && boot.is_empty() && plan.is_empty() && implementation.is_empty() => {
            PatchbayBodyApplicationEntrance::ExternalReader
        }
        2 if !host.is_empty()
            && !boot.is_empty()
            && !plan.is_empty()
            && !implementation.is_empty() =>
        {
            PatchbayBodyApplicationEntrance::Hosted {
                plan_id: PlanId::from(text(plan)?),
                implementation_id: ImplementationId::from(text(implementation)?),
            }
        }
        _ => return Err(ERROR_INPUT),
    };
    let attachment =
        PatchbayBodyAttachment::open_serialized(evidence, entrance).map_err(map_entrance_error)?;
    let (relationship, current_host_id, current_boot_id) = match attachment.entrance() {
        PatchbayBodyApplicationEntrance::ExternalReader => ("external", None, None),
        PatchbayBodyApplicationEntrance::Hosted { .. } => {
            let host_id = HostId::from(text(host)?);
            let boot_id = BootId::from(text(boot)?);
            let current = attachment
                .evidence()
                .membership
                .parts
                .iter()
                .filter_map(|part| part.current.as_ref())
                .find(|observation| {
                    observation.host_id == host_id && observation.boot_id == boot_id
                })
                .ok_or(ERROR_HOSTED_MEMBERSHIP)?;
            (
                "hosted",
                Some(current.host_id.as_str().into()),
                Some(current.boot_id.as_str().into()),
            )
        }
    };
    Ok(EntranceProjection {
        schema: "conduit.patchbay/browser-entrance-projection@1",
        relationship,
        body_id: attachment.projection().body_id.as_str().into(),
        friendly_name: attachment.projection().friendly_name.clone(),
        membership_revision: attachment.evidence().membership.revision.0,
        current_host_id,
        current_boot_id,
        entries: attachment
            .projection()
            .entries
            .iter()
            .map(|entry| EntranceEntry {
                sequence: entry.sequence,
                heading: entry.heading,
                explanation: entry.explanation.clone(),
                evidence_sign_id: entry.evidence_sign_id.as_str().into(),
            })
            .collect(),
    })
}

fn text(bytes: &[u8]) -> Result<&str, i32> {
    let value = core::str::from_utf8(bytes).map_err(|_| ERROR_INPUT)?;
    if value.is_empty() || value.len() > 512 || value.contains(['\0', '\n']) {
        return Err(ERROR_INPUT);
    }
    Ok(value)
}

fn map_entrance_error(_error: PatchbayBodyEntranceError) -> i32 {
    ERROR_EVIDENCE
}

fn clear_output() {
    OUTPUT.with(|output| output.borrow_mut().clear());
}

fn write_output(value: &EntranceProjection) -> i32 {
    let Ok(encoded) = serde_json::to_vec(value) else {
        return ERROR_OUTPUT;
    };
    if encoded.len() > OUTPUT_CAPACITY {
        return ERROR_OUTPUT;
    }
    OUTPUT.with(|output| output.borrow_mut().extend_from_slice(&encoded));
    STATUS_READY
}

#[cfg(test)]
mod tests;
